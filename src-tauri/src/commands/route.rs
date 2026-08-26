//! 模型路由规则命令。

use tauri::State;

use crate::db::route_dao;
use crate::state::AppState;
use crate::types::AppType;

#[tauri::command]
pub fn list_route_rules(
    state: State<'_, AppState>,
    app_type: AppType,
) -> Result<Vec<route_dao::RouteRule>, String> {
    route_dao::list_by_app(&state.db, app_type.as_str()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_route_rule(
    state: State<'_, AppState>,
    app_type: AppType,
    pattern: String,
    provider_id: String,
) -> Result<i64, String> {
    if pattern.trim().is_empty() {
        return Err("匹配词不能为空".into());
    }
    route_dao::insert(&state.db, app_type.as_str(), pattern.trim(), &provider_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_route_rule(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    route_dao::delete(&state.db, id).map_err(|e| e.to_string())
}
