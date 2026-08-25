//! takeover 接管命令。

use tauri::State;

use crate::services::takeover as svc;
use crate::services::takeover::TakeoverStatus;
use crate::state::AppState;
use crate::types::AppType;

#[tauri::command]
pub fn takeover_status(state: State<'_, AppState>) -> Result<Vec<TakeoverStatus>, String> {
    Ok(svc::status(&state.db))
}

#[tauri::command]
pub fn apply_takeover(state: State<'_, AppState>, app_type: AppType) -> Result<(), String> {
    svc::apply(&state.db, app_type).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn restore_takeover(state: State<'_, AppState>, app_type: AppType) -> Result<(), String> {
    svc::restore(&state.db, app_type).map_err(|e| e.to_string())
}

/// 设置某 app 的故障转移开关
#[tauri::command]
pub fn set_failover(
    state: State<'_, AppState>,
    app_type: AppType,
    enabled: bool,
) -> Result<(), String> {
    crate::db::kv::set(
        &state.db,
        &format!("failover:{}", app_type.as_str()),
        if enabled { "1" } else { "0" },
    )
    .map_err(|e| e.to_string())
}
