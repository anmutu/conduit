//! Profile 命令。

use tauri::State;

use crate::services::profile as svc;
use crate::state::AppState;

#[tauri::command]
pub fn list_profiles(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    svc::list(&state.db).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_profile(
    state: State<'_, AppState>,
    name: String,
) -> Result<usize, String> {
    svc::save(&state.db, &name)
        .map(|apps| apps.len())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn apply_profile(state: State<'_, AppState>, name: String) -> Result<usize, String> {
    svc::apply(&state.db, &name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_profile(state: State<'_, AppState>, name: String) -> Result<(), String> {
    svc::delete(&state.db, &name).map_err(|e| e.to_string())
}
