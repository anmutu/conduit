//! 供应商业务逻辑。
//!
//! 命令层调用这里的函数。核心职责:
//! - CRUD 时同步管理 keychain 中的 API Key
//! - 列表/查询时填充 `has_key`(查 keychain 是否存在,不暴露 Key 本身)
//! - 切换(`switch`)只改 DB 的 `is_current`,代理在转发时按它选供应商 → 天然免重启

use anyhow::Result;

use crate::db::{provider_dao, Pool};
use crate::services::keychain;
use crate::types::{AppType, Provider, ProviderInput};

/// 列出某应用下的全部供应商(附带 `has_key`)。
pub fn list_by_app(pool: &Pool, app: AppType) -> Result<Vec<Provider>> {
    let mut list = provider_dao::list_by_app(pool, app)?;
    for p in &mut list {
        p.has_key = match &p.keychain_id {
            Some(kid) => keychain::has_provider_key(kid),
            None => false,
        };
    }
    Ok(list)
}

/// 列出全部供应商(附带 `has_key`)。
pub fn list_all(pool: &Pool) -> Result<Vec<Provider>> {
    let mut list = provider_dao::list_all(pool)?;
    for p in &mut list {
        p.has_key = match &p.keychain_id {
            Some(kid) => keychain::has_provider_key(kid),
            None => false,
        };
    }
    Ok(list)
}

/// 当前供应商(代理转发用)。
pub fn get_current(pool: &Pool, app: AppType) -> Result<Option<Provider>> {
    let mut p = provider_dao::get_current(pool, app)?;
    if let Some(p) = &mut p {
        p.has_key = match &p.keychain_id {
            Some(kid) => keychain::has_provider_key(kid),
            None => false,
        };
    }
    Ok(p)
}

/// 创建供应商;若提供了 API Key,则写入 keychain。
pub fn create(pool: &Pool, input: ProviderInput) -> Result<Provider> {
    let id = uuid::Uuid::new_v4().to_string();
    // keychain 引用 id 与 provider id 一致,便于定位
    let keychain_id = Some(id.clone());

    if let Some(key) = &input.api_key {
        if !key.is_empty() {
            keychain::store_provider_key(&id, key)?;
        }
    }

    let provider = Provider {
        id,
        app_type: input.app_type,
        name: input.name,
        base_url: input.base_url,
        keychain_id,
        models: input.models,
        is_current: false,
        is_healthy: true,
        sort_index: 0,
        created_at: now_ts(),
        has_key: input.api_key.as_ref().map(|k| !k.is_empty()).unwrap_or(false),
    };
    provider_dao::insert(pool, &provider)?;
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
    let provider = provider_dao::get_by_id(pool, id)?
        .ok_or_else(|| anyhow::anyhow!("供应商不存在: {id}"))?;
    let kid = provider.keychain_id.as_deref().unwrap_or(id);
    keychain::store_provider_key(kid, key)?;
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

fn now_ts() -> i64 {
    chrono::Utc::now().timestamp()
}
