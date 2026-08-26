//! 供应商业务逻辑。
//!
//! 命令层调用这里的函数。核心职责:
//! - CRUD 时同步管理 keychain 中的 API Key
//! - 列表/查询时填充 `has_key`(查 keychain 是否存在,不暴露 Key 本身)
//! - 切换(`switch`)只改 DB 的 `is_current`,代理在转发时按它选供应商 → 天然免重启

use anyhow::Result;

use crate::db::{api_key_dao, provider_dao, Pool};
use crate::services::keychain;
use crate::types::{AppType, Protocol, Provider, ProviderInput};

/// 填充 `has_key`:优先 meta 落库值;未知时查加密库 api_keys;
/// 仍未知且为旧数据时查一次 keychain 并迁移落库,此后不再触碰 keychain。
fn fill_has_key(pool: &Pool, list: &mut [Provider]) {
    for p in list.iter_mut() {
        match p.meta_has_key {
            Some(v) => p.has_key = v,
            None => {
                let v = match api_key_dao::get(pool, &p.id) {
                    Ok(Some(_)) => true,
                    _ => crate::services::keys::load(pool, p)
                        .map(|k| k.is_some())
                        .unwrap_or(false),
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
                crate::services::keys::store(pool, &existing.id, key)?;
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
            crate::services::keys::store(pool, &id, key)?;
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

/// 更新某供应商的 API Key(写入加密库)。
pub fn set_api_key(pool: &Pool, id: &str, key: &str) -> Result<()> {
    let provider =
        provider_dao::get_by_id(pool, id)?.ok_or_else(|| anyhow::anyhow!("供应商不存在: {id}"))?;
    crate::services::keys::store(pool, id, key)?;
    provider_dao::set_meta_has_key(pool, id, true)?;
    let _ = provider;
    Ok(())
}

/// 删除供应商,顺带清理加密库与 keychain 中的 Key。
pub fn delete(pool: &Pool, id: &str) -> Result<()> {
    if let Some(p) = provider_dao::get_by_id(pool, id)? {
        let _ = crate::services::keys::delete(pool, &p);
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

/// 连通性测试结果。
#[derive(Debug, serde::Serialize)]
pub struct TestResult {
    pub ok: bool,
    /// HTTP 状态码(网络失败为 None)
    pub status: Option<u16>,
    pub latency_ms: u64,
    /// 失败时的简要说明
    pub message: String,
}

/// 向供应商对应协议的端点发最小请求测连通。
/// - openai:GET {base}/models(最便宜)
/// - anthropic:POST {base}/v1/messages max_tokens=1(需要至少一个模型名)
async fn test_provider_impl(pool: &Pool, id: &str, app: AppType) -> Result<TestResult> {
    let p = provider_dao::get_by_id(pool, id)?
        .ok_or_else(|| anyhow::anyhow!("供应商不存在: {id}"))?;
    let protocol = app.protocol();
    let base = p
        .endpoint(protocol)
        .map(|s| s.to_string())
        .unwrap_or_else(|| p.base_url.clone());
    let base = base.trim_end_matches('/');
    let key = p
        .keychain_id
        .as_deref()
        .and_then(|kid| keychain::load_provider_key(kid).ok().flatten());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;
    let start = std::time::Instant::now();
    let resp_result = match protocol {
        Protocol::Openai => {
            let mut req = client.get(format!("{base}/models"));
            if let Some(k) = &key {
                req = req.bearer_auth(k);
            }
            req.send().await
        }
        Protocol::Anthropic => {
            let model = p
                .models
                .first()
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("未配置模型,无法测试 Anthropic 端点"))?;
            let mut req = client
                .post(format!("{base}/v1/messages"))
                .header("anthropic-version", "2023-06-01")
                .json(&serde_json::json!({
                    "model": model,
                    "max_tokens": 1,
                    "messages": [{"role": "user", "content": "hi"}],
                }));
            if let Some(k) = &key {
                req = req.header("x-api-key", k);
            }
            req.send().await
        }
        Protocol::Gemini => {
            // Gemini:带 key 查模型列表即可
            let url = if let Some(k) = &key {
                format!("{base}/v1/models?key={k}")
            } else {
                format!("{base}/v1/models")
            };
            client.get(url).send().await
        }
    };
    let latency = start.elapsed().as_millis() as u64;
    match resp_result {
        Ok(resp) => {
            let status = resp.status().as_u16();
            Ok(TestResult {
                ok: status < 500, // 4xx 说明端点通了、多为鉴权/参数问题,仍算可达
                status: Some(status),
                latency_ms: latency,
                message: if status < 400 {
                    String::new()
                } else {
                    format!("HTTP {status}")
                },
            })
        }
        Err(e) => Ok(TestResult {
            ok: false,
            status: None,
            latency_ms: latency,
            message: if e.is_timeout() {
                "超时(10s)".into()
            } else if e.is_connect() {
                "连接失败".into()
            } else {
                e.to_string()
            },
        }),
    }
}

pub async fn test_provider(pool: &Pool, id: &str, app: AppType) -> Result<TestResult> {
    test_provider_impl(pool, id, app).await
}
