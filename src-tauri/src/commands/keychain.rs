//! keychain 自检命令(首启向导用于确认凭证存储可用)。

#[tauri::command]
pub fn keychain_health() -> Result<(), String> {
    crate::services::keychain::health_check().map_err(|e| e.to_string())
}
