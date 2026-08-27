//! Keyway 应用入口。
//!
//! 启动顺序:日志 → keychain 主密钥 → 加密 DB → 代理 → 托盘(含供应商快速切换)。

pub mod commands;
pub mod core;
pub mod db;
pub mod services;
pub mod state;
pub mod types;

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
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // 关闭窗口 = 隐藏到托盘(代理继续运行);真正退出走托盘菜单
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                let _ = window.hide();
                api.prevent_close();
            }
        })
        .setup(|app| {
            // 2. 数据目录
            let db_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&db_dir)?;

            // 1. 主密钥:keychain 优先;授权被拒/不可用时回退到本地文件密钥
            //    (0600 权限,DB 依然 SQLCipher 加密;已用文件密钥开过的库继续用文件密钥,
            //     否则换钥匙会打不开旧库。签名发布后 keychain 稳定可迁回)。
            let key_file = db_dir.join("master.key");
            let marker = db_dir.join("master.key.in-use");
            let use_file_key = marker.exists() || keychain::get_or_create_master_key().is_err();
            let master_key = if use_file_key {
                let key = match std::fs::read_to_string(&key_file) {
                    Ok(k) if k.trim().len() == 64 => k.trim().to_string(),
                    _ => {
                        // 生成新的 32 字节密钥
                        let mut s = String::with_capacity(64);
                        s.push_str(&uuid::Uuid::new_v4().simple().to_string());
                        s.push_str(&uuid::Uuid::new_v4().simple().to_string());
                        std::fs::write(&key_file, &s)?;
                        let mut perms = std::fs::metadata(&key_file)?.permissions();
                        use std::os::unix::fs::PermissionsExt;
                        perms.set_mode(0o600);
                        std::fs::set_permissions(&key_file, perms)?;
                        std::fs::write(&marker, b"1")?;
                        tracing::warn!("keychain 不可用,已回退到本地文件主密钥(0600)");
                        s
                    }
                };
                key
            } else {
                keychain::get_or_create_master_key()?
            };
            let db_path = db_dir.join("conduit.db");
            tracing::info!("数据库路径: {}", db_path.display());
            let pool = db::init_pool(&db_path, &master_key)?;

            // 3. 共享状态 + 启动代理
            let state = state::AppState::with_handle(pool, Some(app.handle().clone()));
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
                .tooltip("Keyway — 本地代理运行中")
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
                        // 每次唤起主界面时刷新托盘菜单(「最近使用」随请求变化)
                        let _ = rebuild_tray_menu(app);
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
            commands::provider::upsert_provider_endpoint,
            commands::provider::remove_provider_endpoint,
            commands::provider::set_provider_key,
            commands::provider::test_provider,
            commands::provider::get_provider_balance,
            commands::settings::get_app_settings,
            commands::settings::set_autostart,
            commands::proxy::proxy_status,
            commands::keychain::keychain_health,
            commands::locale::set_locale,
            commands::usage::get_usage_map,
            commands::usage_dash::get_usage_dashboard,
            commands::usage_dash::get_recent_usage,
            commands::route::list_route_rules,
            commands::route::add_route_rule,
            commands::route::delete_route_rule,
            commands::route::set_route_rule_enabled,
            commands::profile::list_profiles,
            commands::profile::save_profile,
            commands::profile::apply_profile,
            commands::profile::delete_profile,
            commands::tray::refresh_tray,
            commands::backup::export_backup,
            commands::backup::import_backup,
            commands::import::import_existing,
            commands::takeover::takeover_status,
            commands::takeover::apply_takeover,
            commands::takeover::restore_takeover,
            commands::takeover::set_failover,
        ])
        .run(tauri::generate_context!())
        .expect("运行 Keyway 时出错");
}

/// 构建托盘菜单:每个有供应商的应用一个子菜单,点击即切换。
/// 当前项以 "●" 标注。供应商增删改后调用 [`rebuild_tray_menu`] 刷新。
fn build_tray_menu<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let state = app.state::<state::AppState>();
    // 托盘文案随语言(默认中文;en 由前端设置页切换写入)
    let zh = db::kv::get(&state.db, "locale").ok().flatten().as_deref() != Some("en");
    let (show_text, quit_text, recent_text) = if zh {
        ("显示主界面", "退出 Keyway", "最近使用")
    } else {
        ("Show Main Window", "Quit Keyway", "Recent")
    };
    let show = MenuItem::with_id(app, "show", show_text, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_text, true, None::<&str>)?;
    let mut builder = MenuBuilder::new(app).item(&show);

    // 最近使用:按最近请求排序的前几个供应商(点击即切到该分组的它)
    let recents: std::collections::HashMap<String, String> = db::provider_dao::list_all(&state.db)
        .unwrap_or_default()
        .into_iter()
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect();
    let recent_items: Vec<(String, String, String)> = db::usage_dao::recent_providers(&state.db, 5)
        .unwrap_or_default()
        .into_iter()
        .filter(|(pid, app_str)| {
            // 只保留当前仍存在、且该分组下可切换的供应商
            recents.contains_key(pid) && types::AppType::from_str(app_str).is_some()
        })
        .map(|(pid, app_str)| {
            let name = recents.get(&pid).cloned().unwrap_or_default();
            (pid, app_str, name)
        })
        .collect();
    if !recent_items.is_empty() {
        let mut sub = SubmenuBuilder::new(app, recent_text);
        for (pid, app_str, name) in recent_items {
            let item = MenuItem::with_id(
                app,
                format!("switch:{}:{}", app_str, pid),
                format!("{} · {}", name, app_str),
                true,
                None::<&str>,
            )?;
            sub = sub.item(&item);
        }
        builder = builder.item(&sub.build()?);
    }

    for app_type in types::AppType::all() {
        let providers = db::provider_dao::list_by_app(&state.db, *app_type)
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.endpoint(app_type.protocol()).is_some())
            .collect::<Vec<_>>();
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
            let item = MenuItem::with_id(
                app,
                format!("switch:{}:{}", app_type.as_str(), p.id),
                label,
                true,
                None::<&str>,
            )?;
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
