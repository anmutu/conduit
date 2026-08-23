//! 用量查询命令。

use std::collections::HashMap;

use tauri::State;

use crate::db::usage_dao::UsageSummary;
use crate::state::AppState;
use crate::types::AppType;

/// 某应用下各供应商的累计用量。key = provider_id
#[tauri::command]
pub fn get_usage_map(
    state: State<'_, AppState>,
    app_type: AppType,
) -> Result<HashMap<String, UsageSummary>, String> {
    crate::db::usage_dao::summarize_map(&state.db, app_type.as_str()).map_err(|e| e.to_string())
}
