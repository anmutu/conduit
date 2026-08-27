//! 供应商相关命令。

use tauri::State;

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
    state: State<'_, AppState>,
    id: String,
    app_type: AppType,
) -> Result<svc::TestResult, String> {
    svc::test_provider(&state.db, &id, app_type)
        .await
        .map_err(|e| e.to_string())
}

/// 查询供应商余额(骨架:接口未定,统一返回 None,前端不展示)。
#[tauri::command]
pub fn get_provider_balance(
    state: State<'_, AppState>,
    id: String,
) -> Result<Option<String>, String> {
    let _ = (&state.db, &id); // TODO: 接入 CoderPlan 余额接口后实现
    Ok(None)
}
