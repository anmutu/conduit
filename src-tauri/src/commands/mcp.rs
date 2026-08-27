//! MCP 统一管理命令。

use tauri::State;

use crate::db::{mcp_dao, mcp_dao::McpServer};
use crate::state::AppState;

#[tauri::command]
pub fn list_mcp_servers(state: State<'_, AppState>) -> Result<Vec<McpServer>, String> {
    mcp_dao::list(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_mcp_server(state: State<'_, AppState>, mut server: McpServer) -> Result<(), String> {
    let id = server.id.trim().to_string();
    if id.is_empty() {
        return Err("ID 不能为空".into());
    }
    // id 作为各配置文件的键名,只允许安全字符
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("ID 仅支持字母、数字、-、_".into());
    }
    server.id = id;
    mcp_dao::upsert(&state.db, &server).map_err(|e| e.to_string())?;
    crate::services::mcp::sync_all(&state.db).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_mcp_server(state: State<'_, AppState>, id: String) -> Result<(), String> {
    mcp_dao::delete(&state.db, &id).map_err(|e| e.to_string())?;
    crate::services::mcp::sync_all(&state.db).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn set_mcp_server_enabled(
    state: State<'_, AppState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    mcp_dao::set_enabled(&state.db, &id, enabled).map_err(|e| e.to_string())?;
    crate::services::mcp::sync_all(&state.db).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn sync_mcp_servers(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    crate::services::mcp::sync_all(&state.db).map_err(|e| e.to_string())
}
