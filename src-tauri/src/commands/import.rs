//! 首启导入命令。

use tauri::State;

use crate::services::import::{self, ImportedProvider};
use crate::state::AppState;

#[tauri::command]
pub fn import_existing(state: State<'_, AppState>) -> Result<Vec<ImportedProvider>, String> {
    import::import_existing(&state.db).map_err(|e| e.to_string())
}
