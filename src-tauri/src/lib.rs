//! Conduit 应用入口。
//!
//! 启动顺序:
//! 1. 初始化日志
//! 2. 从 keychain 取/建 SQLCipher 主密钥
//! 3. 打开加密 DB 并建表
//! 4. 构造共享 `AppState`,启动代理(独立 tokio task)
//! 5. 注册命令、管理状态、运行 Tauri

mod commands;
mod core;
mod db;
mod services;
mod state;
mod types;

use core::proxy::{server, PROXY_ADDR};
use services::keychain;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "conduit=info,tower_http=info".into()),
        )
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // 1. 主密钥(必要时首次生成)
            let master_key = keychain::get_or_create_master_key().map_err(|e| {
                tracing::error!("keychain 不可用: {e}");
                e
            })?;

            // 2. 加密 DB
            let db_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&db_dir)?;
            let db_path = db_dir.join("conduit.db");
            tracing::info!("数据库路径: {}", db_path.display());
            let pool = db::init_pool(&db_path, &master_key)?;

            // 3. 共享状态 + 启动代理
            let state = state::AppState::new(pool);
            let proxy_state = state.clone();
            let addr = PROXY_ADDR.to_string();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = server::run(proxy_state, &addr).await {
                    tracing::error!("代理服务退出: {e}");
                }
            });

            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::provider::list_providers,
            commands::provider::list_all_providers,
            commands::provider::get_current_provider,
            commands::provider::create_provider,
            commands::provider::switch_provider,
            commands::provider::delete_provider,
            commands::provider::update_provider,
            commands::provider::set_provider_key,
            commands::proxy::proxy_status,
            commands::keychain::keychain_health,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Conduit 时出错");
}
