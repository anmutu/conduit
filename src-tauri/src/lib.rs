//! Conduit 应用入口。
//!
//! 启动顺序:日志 → keychain 主密钥 → 加密 DB → 代理 → 托盘(含供应商快速切换)。

mod commands;
mod core;
mod db;
mod services;
mod state;
mod types;

use core::proxy::{server, PROXY_ADDR};
use services::keychain;
use tauri::menu::{Menu, MenuBuilder, MenuItem, SubmenuBuilder};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, Runtime, WindowEvent};

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
        // 关闭窗口 = 隐藏到托盘(代理继续运行);真正退出走托盘菜单
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
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

            // 4. 系统托盘:供应商快速切换 + 显示主界面 / 退出
            let menu = build_tray_menu(app.handle())?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().expect("缺少应用图标").clone())
                .tooltip("Conduit — 本地代理运行中")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(on_tray_menu_event)
                .on_tray_icon_event(|tray, event| {
                    // 左键单击:显示主窗口
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                })
                .build(app)?;

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
            commands::tray::refresh_tray,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Conduit 时出错");
}

/// 构建托盘菜单:每个有供应商的应用一个子菜单,点击即切换。
/// 当前项以 "●" 标注。供应商增删改后调用 [`rebuild_tray_menu`] 刷新。
fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let state = app.state::<state::AppState>();
    let show = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Conduit", true, None::<&str>)?;
    let mut builder = MenuBuilder::new(app).item(&show);

    for app_type in types::AppType::all() {
        let providers = db::provider_dao::list_by_app(&state.db, app_type)
            .unwrap_or_default();
        if providers.is_empty() {
            continue;
        }
        let mut sub = SubmenuBuilder::new(app, app_type.as_str());
        for p in providers {
            let label = if p.is_current {
                format!("● {}", p.name)
            } else {
                p.name.clone()
            };
            // id 格式:switch:{app_type}:{provider_id}
            let item =
                MenuItem::with_id(app, format!("switch:{}:{}", app_type.as_str(), p.id), label, true, None::<&str>)?;
            sub = sub.item(&item);
        }
        builder = builder.item(&sub.build()?);
    }

    builder.item(&quit).build()
}

/// 用最新数据重建托盘菜单。
pub fn rebuild_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    if let Some(tray) = app.tray_by_id("main") {
        tray.set_menu(Some(build_tray_menu(app)?))?;
    }
    Ok(())
}

/// 托盘菜单事件:show/quit/switch:{app}:{provider_id}
fn on_tray_menu_event<R: Runtime>(app: &AppHandle<R>, event: tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        "show" => {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }
        "quit" => app.exit(0),
        id => {
            if let Some(rest) = id.strip_prefix("switch:") {
                let mut parts = rest.splitn(2, ':');
                let app_str = parts.next().unwrap_or("");
                let provider_id = parts.next().unwrap_or("");
                let Some(app_type) = types::AppType::from_str(app_str) else {
                    return;
                };
                let state = app.state::<state::AppState>();
                if services::provider::switch(&state.db, provider_id, app_type).is_err() {
                    return;
                }
                let name = db::provider_dao::get_by_id(&state.db, provider_id)
                    .ok()
                    .flatten()
                    .map(|p| p.name)
                    .unwrap_or_default();
                // 通知前端刷新(托盘切换后 UI 同步)
                let _ = app.emit(
                    "provider-switched",
                    serde_json::json!({ "appType": app_str, "providerId": provider_id, "name": name }),
                );
                // 当前项标注变了,重建菜单
                if let Err(e) = rebuild_tray_menu(app) {
                    tracing::warn!("重建托盘菜单失败: {e}");
                }
            }
        }
    }
}

