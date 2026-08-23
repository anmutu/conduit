//! 设置相关命令。

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;

#[derive(Debug, Serialize)]
pub struct AppSettings {
    /// 开机自启动
    pub autostart: bool,
    /// 数据库文件路径(展示用)
    pub db_path: String,
    /// 本地代理地址
    pub proxy_addr: String,
}

#[tauri::command]
pub fn get_app_settings(app: AppHandle) -> Result<AppSettings, String> {
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    let db_path = app
        .path()
        .app_data_dir()
        .map(|d| d.join("conduit.db").display().to_string())
        .unwrap_or_default();
    Ok(AppSettings {
        autostart,
        db_path,
        proxy_addr: crate::core::proxy::PROXY_ADDR.to_string(),
    })
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())
    }
}
