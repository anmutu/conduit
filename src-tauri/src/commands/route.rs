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
    match_type: Option<String>,
) -> Result<i64, String> {
    if pattern.trim().is_empty() {
        return Err("匹配词不能为空".into());
    }
    let mt = match_type.as_deref().unwrap_or("contains");
    if !matches!(mt, "contains" | "starts_with") {
        return Err("match_type 仅支持 contains / starts_with".into());
    }
    route_dao::insert(
        &state.db,
        app_type.as_str(),
        pattern.trim(),
        &provider_id,
        mt,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_route_rule(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    route_dao::delete(&state.db, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_route_rule_enabled(
    state: State<'_, AppState>,
    id: i64,
    enabled: bool,
) -> Result<(), String> {
    route_dao::set_enabled(&state.db, id, enabled).map_err(|e| e.to_string())
}

/// 设置规则级降级供应商;provider_id 为 None/空 = 清除。
#[tauri::command]
pub fn set_route_rule_fallback(
    state: State<'_, AppState>,
    id: i64,
    provider_id: Option<String>,
) -> Result<(), String> {
    let fb = provider_id.filter(|s| !s.trim().is_empty());
    route_dao::set_fallback(&state.db, id, fb.as_deref()).map_err(|e| e.to_string())
}

/// 长上下文分流预设(某 app)。None = 未配置。
#[tauri::command]
pub fn get_longctx_preset(
    state: State<'_, AppState>,
    app_type: AppType,
) -> Result<Option<serde_json::Value>, String> {
    let raw = crate::db::kv::get(&state.db, &format!("route.longctx.{}", app_type.as_str()))
        .map_err(|e| e.to_string())?;
    Ok(raw.and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok()))
}

/// 保存长上下文预设;provider_id 为空表示清除。
#[tauri::command]
pub fn set_longctx_preset(
    state: State<'_, AppState>,
    app_type: AppType,
    provider_id: Option<String>,
    threshold: Option<i64>,
) -> Result<(), String> {
    let pid = provider_id.unwrap_or_default();
    if pid.is_empty() {
        return route_dao::clear_longctx(&state.db, app_type.as_str()).map_err(|e| e.to_string());
    }
    let th = threshold.unwrap_or(60_000).clamp(1_000, 2_000_000);
    route_dao::set_longctx(&state.db, app_type.as_str(), &pid, th).map_err(|e| e.to_string())
}

/// 后台轻量分流预设(某 app)。None = 未配置。
#[tauri::command]
pub fn get_background_preset(
    state: State<'_, AppState>,
    app_type: AppType,
) -> Result<Option<String>, String> {
    route_dao::get_background(&state.db, app_type.as_str()).map_err(|e| e.to_string())
}

/// 保存后台轻量预设;provider_id 为空表示清除。
#[tauri::command]
pub fn set_background_preset(
    state: State<'_, AppState>,
    app_type: AppType,
    provider_id: Option<String>,
) -> Result<(), String> {
    let pid = provider_id.unwrap_or_default();
    if pid.is_empty() {
        return route_dao::clear_background(&state.db, app_type.as_str())
            .map_err(|e| e.to_string());
    }
    route_dao::set_background(&state.db, app_type.as_str(), &pid).map_err(|e| e.to_string())
}

/// 上移/下移路由规则(与相邻规则交换优先级)。dir = -1 上移 / +1 下移。
#[tauri::command]
pub fn move_route_rule(state: State<'_, AppState>, id: i64, dir: i64) -> Result<(), String> {
    route_dao::move_rule(&state.db, id, dir).map_err(|e| e.to_string())
}
