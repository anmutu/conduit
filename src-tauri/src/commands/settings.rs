//! 设置相关命令。

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::state::AppState;
use tauri_plugin_autostart::ManagerExt;

#[derive(Debug, Serialize)]
pub struct AppSettings {
    /// 开机自启动
    pub autostart: bool,
    /// 数据库文件路径(展示用)
    pub db_path: String,
    /// 本地代理地址
    pub proxy_addr: String,
    /// 请求日志保留天数
    pub retention_days: i64,
    /// 距上次备份天数(None=从未备份)
    pub days_since_backup: Option<i64>,
}

#[tauri::command]
pub fn get_app_settings(app: AppHandle, state: State<'_, AppState>) -> Result<AppSettings, String> {
    let autostart = app.autolaunch().is_enabled().unwrap_or(false);
    let retention_days = usage_retention_days(&state.db);
    let days_since_backup = crate::db::kv::get(&state.db, "backup.last_at")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .map(|ts| (chrono::Utc::now().timestamp() - ts) / 86400);
    let db_path = app
        .path()
        .app_data_dir()
        .map(|d| d.join("conduit.db").display().to_string())
        .unwrap_or_default();
    Ok(AppSettings {
        autostart,
        db_path,
        proxy_addr: crate::core::proxy::PROXY_ADDR.to_string(),
        retention_days,
        days_since_backup,
    })
}

/// 日志保留天数(KV `usage.retention_days`,默认 30,范围 1..=365)
pub fn usage_retention_days(pool: &crate::db::Pool) -> i64 {
    crate::db::kv::get(pool, "usage.retention_days")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30)
        .clamp(1, 365)
}

/// 设置请求日志保留天数
#[tauri::command]
pub fn set_usage_retention(state: State<'_, AppState>, days: i64) -> Result<(), String> {
    let days = days.clamp(1, 365);
    crate::db::kv::set(&state.db, "usage.retention_days", &days.to_string())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_days_defaults_and_clamps() {
        let key = "ab".repeat(32);
        let dir = std::env::temp_dir().join(format!("keyway_kv_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let pool = crate::db::init_pool(dir.join("t.db"), &key).unwrap();
        // 未设置 → 默认 30
        assert_eq!(usage_retention_days(&pool), 30);
        // 越界值收敛到 1..=365
        crate::db::kv::set(&pool, "usage.retention_days", "0").unwrap();
        assert_eq!(usage_retention_days(&pool), 1);
        crate::db::kv::set(&pool, "usage.retention_days", "9000").unwrap();
        assert_eq!(usage_retention_days(&pool), 365);
        // 合法值原样返回;垃圾值回落默认
        crate::db::kv::set(&pool, "usage.retention_days", "14").unwrap();
        assert_eq!(usage_retention_days(&pool), 14);
        crate::db::kv::set(&pool, "usage.retention_days", "abc").unwrap();
        assert_eq!(usage_retention_days(&pool), 30);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// 周期健康检测间隔(分钟;0=关闭,默认)。写入后由后台线程轮询生效。
#[tauri::command]
pub fn set_health_interval(state: State<'_, AppState>, minutes: i64) -> Result<(), String> {
    let m = minutes.clamp(0, 24 * 60);
    if m == 0 {
        crate::db::kv::del(&state.db, "health.interval_min").map_err(|e| e.to_string())
    } else {
        crate::db::kv::set(&state.db, "health.interval_min", &m.to_string())
            .map_err(|e| e.to_string())
    }
}

/// 读取周期健康检测间隔(None=关闭)。
#[tauri::command]
pub fn get_health_interval(state: State<'_, AppState>) -> Result<Option<i64>, String> {
    Ok(crate::db::kv::get(&state.db, "health.interval_min")
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse::<i64>().ok()))
}
