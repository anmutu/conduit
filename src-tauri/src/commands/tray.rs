//! 托盘相关命令。

/// 供应商增删改后由前端调用,重建托盘快速切换菜单。
#[tauri::command]
pub fn refresh_tray(app: tauri::AppHandle) -> Result<(), String> {
    crate::rebuild_tray_menu(&app).map_err(|e| e.to_string())
}
