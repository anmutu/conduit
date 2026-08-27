//! 配置备份命令。

use tauri::{AppHandle, Manager, State};

use crate::services::backup as svc;
use crate::state::AppState;

/// 导出全部供应商配置到 app 数据目录,返回文件路径与数量。
#[tauri::command]
pub fn export_backup(app: AppHandle, state: State<'_, AppState>) -> Result<ExportResult, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位数据目录: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = svc::default_backup_path(&dir);
    let count = svc::export(&state.db, &path).map_err(|e| e.to_string())?;
    Ok(ExportResult {
        path: path.to_string_lossy().into_owned(),
        count,
    })
}

#[derive(serde::Serialize)]
pub struct ExportResult {
    pub path: String,
    pub count: usize,
}

/// 从备份导入(同名跳过),返回 (新建, 跳过)。
/// path 为空时自动取数据目录里最新的 conduit-backup-*.json。
#[tauri::command]
pub fn import_backup(
    app: AppHandle,
    state: State<'_, AppState>,
    path: Option<String>,
) -> Result<(usize, usize), String> {
    let file = match path {
        Some(p) => std::path::PathBuf::from(p),
        None => {
            let dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("无法定位数据目录: {e}"))?;
            let mut newest: Option<std::path::PathBuf> = None;
            let entries = std::fs::read_dir(&dir).map_err(|e| e.to_string())?;
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with("conduit-backup-") && name.ends_with(".json") {
                    if newest.as_ref().is_none_or(|n| e.path() > n.clone()) {
                        newest = Some(e.path());
                    }
                }
            }
            newest.ok_or_else(|| "未找到备份文件,请先导出".to_string())?
        }
    };
    svc::import(&state.db, &file).map_err(|e| e.to_string())
}
