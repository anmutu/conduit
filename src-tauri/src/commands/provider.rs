//! 供应商相关命令。

use tauri::State;

use crate::db::provider_dao;
use crate::services::provider as svc;
use crate::state::AppState;
use crate::types::{AppType, Protocol, Provider, ProviderInput};

#[tauri::command]
pub fn list_providers(
    state: State<'_, AppState>,
    app_type: AppType,
) -> Result<Vec<Provider>, String> {
    svc::list_by_app(&state.db, app_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn list_all_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    svc::list_all(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_current_provider(
    state: State<'_, AppState>,
    app_type: AppType,
) -> Result<Option<Provider>, String> {
    svc::get_current(&state.db, app_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_provider(
    state: State<'_, AppState>,
    input: ProviderInput,
) -> Result<Provider, String> {
    svc::create(&state.db, input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn switch_provider(
    state: State<'_, AppState>,
    id: String,
    app_type: AppType,
) -> Result<(), String> {
    svc::switch(&state.db, &id, app_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_provider(state: State<'_, AppState>, id: String) -> Result<(), String> {
    svc::delete(&state.db, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_provider(state: State<'_, AppState>, id: String, name: String) -> Result<(), String> {
    svc::update(&state.db, &id, &name).map_err(|e| e.to_string())
}

/// 新增/更新某协议端点(anthropic / openai / gemini)
#[tauri::command]
pub fn upsert_provider_endpoint(
    state: State<'_, AppState>,
    id: String,
    protocol: Protocol,
    base_url: String,
) -> Result<(), String> {
    svc::upsert_endpoint(&state.db, &id, protocol, &base_url).map_err(|e| e.to_string())
}

/// 移除某协议端点(至少保留一个)
#[tauri::command]
pub fn remove_provider_endpoint(
    state: State<'_, AppState>,
    id: String,
    protocol: Protocol,
) -> Result<(), String> {
    svc::remove_endpoint(&state.db, &id, protocol).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_provider_key(
    state: State<'_, AppState>,
    id: String,
    api_key: String,
) -> Result<(), String> {
    svc::set_api_key(&state.db, &id, &api_key).map_err(|e| e.to_string())
}

/// 连通性测试:向该供应商对应协议端点发一个最小请求,返回状态/延迟。
#[tauri::command]
pub async fn test_provider(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: String,
    app_type: AppType,
) -> Result<svc::TestResult, String> {
    let r = svc::test_provider(&state.db, &id, app_type)
        .await
        .map_err(|e| e.to_string())?;
    // 测速结果落库(meta.last_test)→ 托盘 ✗ 标记即时刷新
    let _ = crate::rebuild_tray_menu(&app);
    Ok(r)
}

/// 批量连通性测试:该分组全部供应商并发测速。
#[tauri::command]
pub async fn test_all_providers(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    app_type: AppType,
) -> Result<Vec<svc::BatchTestItem>, String> {
    let r = svc::test_all_providers(&state.db, app_type)
        .await
        .map_err(|e| e.to_string())?;
    let _ = crate::rebuild_tray_menu(&app);
    Ok(r)
}

/// Responses 桥接开关:开启后该供应商把 Codex 的 /v1/responses 请求
/// 转成 chat/completions 转发(适用于不支持 Responses API 的中转站)。
#[tauri::command]
pub fn set_responses_bridge(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    let key = format!("responses.bridge.{id}");
    if enabled {
        crate::db::kv::set(&state.db, &key, "1").map_err(|e| e.to_string())
    } else {
        crate::db::kv::del(&state.db, &key).map_err(|e| e.to_string())
    }
}

/// 拖拽排序:按 id 顺序写 sort_index
#[tauri::command]
pub fn reorder_providers(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    crate::db::provider_dao::reorder(&state.db, &ids).map_err(|e| e.to_string())
}

/// 读取 /v1/responses 桥接开关(某供应商)。
#[tauri::command]
pub fn get_responses_bridge(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    Ok(
        crate::db::kv::get(&state.db, &format!("responses.bridge.{id}"))
            .map_err(|e| e.to_string())?
            .as_deref()
            == Some("1"),
    )
}

/// 供应商余额(OpenRouter 兼容 /api/v1/key;其他上游返回 not_supported)。
#[tauri::command]
pub async fn get_provider_balance(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let (pool, provider) = {
        let db = state.inner().db.clone();
        let p = provider_dao::get_by_id(&db, &id).map_err(|e| e.to_string())?;
        (db, p)
    };
    let p = provider.ok_or("provider not found")?;
    let key = crate::services::keys::load_async(&pool, &p).await;
    let key = key.filter(|k| !k.is_empty()).ok_or("no api key")?;
    let base = p
        .endpoints
        .get("openai")
        .cloned()
        .unwrap_or_else(|| p.base_url.clone());
    let url = format!("{}/api/v1/key", base.trim_end_matches('/'));
    let resp = reqwest::Client::new()
        .get(&url)
        .bearer_auth(&key)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let v: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("HTTP {status}"));
    }
    let d = v.get("data").cloned().ok_or("not_supported")?;
    Ok(serde_json::json!({
        "label": d.get("label").cloned().unwrap_or(serde_json::Value::Null),
        "usage": d.get("usage").and_then(|x| x.as_f64()),
        "limit": d.get("limit").and_then(|x| x.as_f64()),
        "is_free_tier": d.get("is_free_tier").and_then(|x| x.as_bool()).unwrap_or(false),
    }))
}

/// 更新供应商模型列表(逗号分隔字符串)。
#[tauri::command]
pub fn set_provider_models(
    state: State<'_, AppState>,
    id: String,
    models: Vec<String>,
) -> Result<(), String> {
    provider_dao::set_models(&state.db, &id, &models).map_err(|e| e.to_string())
}

/// 故障转移候选链(名称序):仅当该分组开启 failover 时返回,否则空数组。
#[tauri::command]
pub fn get_failover_chain(
    state: State<'_, AppState>,
    app_type: AppType,
) -> Result<Vec<String>, String> {
    let on = crate::db::kv::get(&state.db, &format!("failover:{}", app_type.as_str()))
        .map_err(|e| e.to_string())?
        .as_deref()
        == Some("1");
    if !on {
        return Ok(vec![]);
    }
    Ok(provider_dao::failover_candidates(&state.db, app_type)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|p| p.name)
        .collect())
}
