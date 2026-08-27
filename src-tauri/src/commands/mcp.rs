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

/// 测试 MCP 服务器连接:远程(http/sse)探测 URL 可达性与状态码;
/// stdio 检查可执行文件是否存在(PATH 中查找)。
#[tauri::command]
pub async fn test_mcp_server(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let server = mcp_dao::list(&state.db)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "MCP 服务器不存在".to_string())?;
    let cfg = &server.config;
    if let Some(url) = cfg.get("url").and_then(|v| v.as_str()) {
        if url.starts_with("http://") || url.starts_with("https://") {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .map_err(|e| e.to_string())?;
            let start = std::time::Instant::now();
            let resp = client
                .head(url)
                .send()
                .await
                .map_err(|e| format!("连接失败: {e}"))?;
            let ms = start.elapsed().as_millis();
            return Ok(format!("HTTP {} · {}ms", resp.status().as_u16(), ms));
        }
        return Err("URL 协议不支持(仅 http/https)".into());
    }
    if let Some(cmd) = cfg.get("command").and_then(|v| v.as_str()) {
        let found = which_cmd(cmd);
        return if let Some(path) = found {
            Ok(format!("可执行: {path}"))
        } else {
            Err(format!("未找到可执行文件: {cmd}(检查 PATH)"))
        };
    }
    Err("配置缺少 url 或 command".into())
}

fn which_cmd(cmd: &str) -> Option<String> {
    if cmd.contains('/') {
        let p = std::path::Path::new(cmd);
        return (p.exists()).then(|| cmd.to_string());
    }
    let path = std::env::var("PATH").ok()?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(cmd))
        .find(|p| p.exists())
        .map(|p| p.display().to_string())
}
