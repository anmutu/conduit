//! 供应商业务逻辑。
//!
//! 命令层调用这里的函数。核心职责:
//! - CRUD 时同步管理 keychain 中的 API Key
//! - 列表/查询时填充 `has_key`(查 keychain 是否存在,不暴露 Key 本身)
//! - 切换(`switch`)只改 DB 的 `is_current`,代理在转发时按它选供应商 → 天然免重启

use anyhow::Result;

use crate::db::{provider_dao, Pool};
use crate::services::keychain;
use crate::types::{AppType, Protocol, Provider, ProviderInput};

/// 填充 `has_key`:优先用 meta 落库值(启动零 keychain 访问,不弹授权框);
/// 旧数据无 meta 时查一次 keychain 并**回写 meta 自愈**,此后不再触碰。
fn fill_has_key(pool: &Pool, list: &mut [Provider]) {
    for p in list.iter_mut() {
        match p.meta_has_key {
            Some(v) => p.has_key = v,
            None => {
                let v = match &p.keychain_id {
                    Some(kid) => keychain::has_provider_key(kid),
                    None => false,
                };
                p.has_key = v;
                let _ = provider_dao::set_meta_has_key(pool, &p.id, v);
            }
        }
    }
}

/// 列出某应用下的全部供应商(附带 `has_key`)。
pub fn list_by_app(pool: &Pool, app: AppType) -> Result<Vec<Provider>> {
    let mut list = provider_dao::list_by_app(pool, app)?;
    fill_has_key(pool, &mut list);
    Ok(list)
}

/// 列出全部供应商(附带 `has_key`)。
pub fn list_all(pool: &Pool) -> Result<Vec<Provider>> {
    let mut list = provider_dao::list_all(pool)?;
    fill_has_key(pool, &mut list);
    Ok(list)
}

/// 当前供应商(代理转发用)。
pub fn get_current(pool: &Pool, app: AppType) -> Result<Option<Provider>> {
    let mut p = provider_dao::get_current(pool, app)?;
    if let Some(p) = &mut p {
        fill_has_key(pool, std::slice::from_mut(p));
    }
    Ok(p)
}

/// 创建供应商;若同名供应商已存在则**合并**:只追加当前分组协议的端点(不新建)。
/// 提供了 API Key 时写入 keychain(已有 Key 且新值为空则保留旧 Key)。
pub fn create(pool: &Pool, input: ProviderInput) -> Result<Provider> {
    // 同名合并:补端点即可
    if let Some(existing) = provider_dao::find_by_name(pool, &input.name)? {
        let protocol = input.app_type.protocol();
        if !input.base_url.trim().is_empty() {
            provider_dao::upsert_endpoint(pool, &existing.id, protocol, &input.base_url)?;
        }
        let mut key_stored = false;
        if let Some(key) = &input.api_key {
            if !key.is_empty() {
                keychain::store_provider_key(existing.keychain_id.as_deref().unwrap_or(&existing.id), key)?;
                key_stored = true;
            }
        }
        if key_stored {
            let _ = provider_dao::set_meta_has_key(pool, &existing.id, true);
        }
        let mut merged = provider_dao::get_by_id(pool, &existing.id)?
            .ok_or_else(|| anyhow::anyhow!("供应商合并失败: {}", input.name))?;
        merged.has_key = key_stored || existing.has_key;
        // 该分组还没有当前供应商 → 自动设为当前(新增即生效,免二次点击)
        if provider_dao::get_current(pool, input.app_type)?.is_none() {
            provider_dao::set_current(pool, &merged.id, input.app_type)?;
            merged.is_current = true;
        }
        return Ok(merged);
    }

    let id = uuid::Uuid::new_v4().to_string();
    // keychain 引用 id 与 provider id 一致,便于定位
    let keychain_id = Some(id.clone());

    if let Some(key) = &input.api_key {
        if !key.is_empty() {
            keychain::store_provider_key(&id, key)?;
        }
    }

    let protocol = input.app_type.protocol();
    let mut endpoints = std::collections::HashMap::new();
    endpoints.insert(protocol.as_str().to_string(), input.base_url.clone());
    let mut provider = Provider {
        id,
        app_type: input.app_type,
        name: input.name,
        base_url: input.base_url,
        endpoints,
        keychain_id,
        models: input.models,
        is_current: false,
        is_healthy: true,
        sort_index: 0,
        created_at: now_ts(),
        has_key: input
            .api_key
            .as_ref()
            .map(|k| !k.is_empty())
            .unwrap_or(false),
        meta_has_key: None, // insert 会按 has_key 写入 meta
    };
    provider_dao::insert(pool, &provider)?;
    // 该分组还没有当前供应商 → 自动设为当前(新增即生效,免二次点击)
    if provider_dao::get_current(pool, input.app_type)?.is_none() {
        provider_dao::set_current(pool, &provider.id, input.app_type)?;
        provider.is_current = true;
    }
    Ok(provider)
}

/// 切换当前供应商(同 app_type 仅一个 current)。
///
/// 因为代理转发按 `is_current` 选供应商,这里只改 DB —— 对所有 CLI 立即生效,
/// 无需写 live 配置文件、无需重启终端。这是 Conduit 的核心体验差异化。
pub fn switch(pool: &Pool, id: &str, app: AppType) -> Result<()> {
    provider_dao::set_current(pool, id, app)
}

/// 更新某供应商的 API Key(写入 keychain)。
pub fn set_api_key(pool: &Pool, id: &str, key: &str) -> Result<()> {
    let provider =
        provider_dao::get_by_id(pool, id)?.ok_or_else(|| anyhow::anyhow!("供应商不存在: {id}"))?;
    let kid = provider.keychain_id.as_deref().unwrap_or(id);
    keychain::store_provider_key(kid, key)?;
    provider_dao::set_meta_has_key(pool, id, true)?;
    Ok(())
}

/// 删除供应商,顺带清理 keychain 中的 Key。
pub fn delete(pool: &Pool, id: &str) -> Result<()> {
    if let Some(p) = provider_dao::get_by_id(pool, id)? {
        if let Some(kid) = &p.keychain_id {
            let _ = keychain::delete_provider_key(kid);
        }
    }
    provider_dao::delete(pool, id)
}

/// 更新供应商名称。
pub fn update(pool: &Pool, id: &str, name: &str) -> Result<()> {
    provider_dao::update(pool, id, name)
}

/// 新增/更新某协议端点。
pub fn upsert_endpoint(
    pool: &Pool,
    id: &str,
    protocol: Protocol,
    base_url: &str,
) -> Result<()> {
    provider_dao::upsert_endpoint(pool, id, protocol, base_url)
}

/// 移除某协议端点(至少保留一个)。
pub fn remove_endpoint(pool: &Pool, id: &str, protocol: Protocol) -> Result<()> {
    provider_dao::remove_endpoint(pool, id, protocol)
}

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}
