//! Skills 统一管理命令。

use tauri::State;

use crate::services::skills;
use crate::state::AppState;

#[tauri::command]
pub fn list_skills(state: State<'_, AppState>) -> Result<Vec<skills::SkillEntry>, String> {
    let _ = state;
    skills::list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_skill(id: String, content: String, apps: Vec<String>) -> Result<(), String> {
    skills::save(&id, &content, &apps).map_err(|e| e.to_string())?;
    skills::sync_all().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn delete_skill(id: String) -> Result<(), String> {
    skills::delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn import_skill(app: String, id: String) -> Result<(), String> {
    skills::import_from(&app, &id).map_err(|e| e.to_string())?;
    skills::sync_all().map_err(|e| e.to_string())?;
    Ok(())
}

/// 列出某 CLI skills 目录下可导入的 skill id
#[tauri::command]
pub fn scan_cli_skills(app: String) -> Result<Vec<String>, String> {
    skills::scan_cli(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn sync_skills() -> Result<Vec<String>, String> {
    skills::sync_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn read_skill_content(id: String) -> Result<String, String> {
    skills::read_content(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_skill_apps(id: String, apps: Vec<String>) -> Result<(), String> {
    skills::set_apps(&id, &apps).map_err(|e| e.to_string())?;
    skills::sync_all().map_err(|e| e.to_string())?;
    Ok(())
}
