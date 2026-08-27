//! 用量仪表盘命令。

use tauri::State;

use crate::db::usage_dao::UsageDashboard;
use crate::state::AppState;
use crate::types::AppType;

#[tauri::command]
pub fn get_usage_dashboard(
    state: State<'_, AppState>,
    app_type: AppType,
    days: Option<i64>,
) -> Result<UsageDashboard, String> {
    let days = days.unwrap_or(7).clamp(1, 90);
    crate::db::usage_dao::dashboard(&state.db, app_type.as_str(), days).map_err(|e| e.to_string())
}

/// 最近请求日志(请求浏览器)
#[tauri::command]
pub fn get_recent_usage(
    state: State<'_, AppState>,
    app_type: AppType,
    limit: Option<i64>,
) -> Result<Vec<crate::db::usage_dao::UsageEntry>, String> {
    crate::db::usage_dao::recent(&state.db, app_type.as_str(), limit.unwrap_or(100))
        .map_err(|e| e.to_string())
}
