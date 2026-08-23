//! 语言设置:前端切换时同步托盘菜单文案。

use tauri::{AppHandle, State};

use crate::state::AppState;

#[tauri::command]
pub fn set_locale(
    app: AppHandle,
    state: State<'_, AppState>,
    locale: String,
) -> Result<(), String> {
    if locale != "zh" && locale != "en" && locale != "system" {
        return Err("locale 必须是 zh/en/system".into());
    }
    crate::db::kv::set(&state.db, "locale", &locale).map_err(|e| e.to_string())?;
    // system → 按系统语言解析
    let effective = if locale == "system" {
        let is_zh = sys_locale_zh();
        if is_zh { "zh" } else { "en" }.to_string()
    } else {
        locale
    };
    crate::db::kv::set(&state.db, "locale", &effective).map_err(|e| e.to_string())?;
    // 重建托盘(文案随语言)
    crate::rebuild_tray_menu(&app).map_err(|e| e.to_string())?;
    Ok(())
}

fn sys_locale_zh() -> bool {
    std::env::var("LANG")
        .map(|v| v.starts_with("zh"))
        .unwrap_or(true)
}
