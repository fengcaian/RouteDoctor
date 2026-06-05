// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod commands;
pub mod services;
pub mod models;
pub mod storage;
pub mod utils;
pub mod error;

use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager, WindowEvent};
use tauri::tray::{TrayIconBuilder, TrayIconEvent, MouseButton};
use tauri::menu::{MenuBuilder, MenuItemBuilder};
use tauri_plugin_autostart::MacosLauncher;

/// 应用级 state：持有 trace 持久化 writer 的 mpsc Sender。
/// 各路径监控会话克隆此 Sender 后向 writer 推送样本。
pub struct TracePersistState(pub tokio::sync::mpsc::Sender<storage::trace_persist::PersistEvent>);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logger for debug builds
    #[cfg(debug_assertions)]
    {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--minimized"]),
        ))
        .setup(|app| {
            // Initialize database
            let app_handle = app.handle();
            storage::database::init_database(&app_handle)?;

            // Initialize native ICMP engine (surge-ping). Must be done inside the
            // Tokio runtime since surge_ping::Client::new() registers an IO source.
            tauri::async_runtime::block_on(async {
                services::icmp_engine::init();
            });

            // Spawn the trace persistence writer task. The Sender is stored as app state
            // so trace tasks can clone it on demand.
            let persist_tx = storage::trace_persist::spawn_writer(app_handle.clone());
            app.manage(TracePersistState(persist_tx));

            // Build tray menu
            let show_item = MenuItemBuilder::with_id("show", "显示主窗口").build(app)?;
            let pause_item = MenuItemBuilder::with_id("pause", "暂停所有 Ping").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "退出").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&show_item)
                .item(&pause_item)
                .separator()
                .item(&quit_item)
                .build()?;

            // Build tray icon
            let app_handle_for_tray = app.handle().clone();
            let _tray = TrayIconBuilder::with_id("main")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    match event.id().as_ref() {
                        "show" => {
                            if let Some(window) = app.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                        "pause" => {
                            let ah = app.clone();
                            tauri::async_runtime::spawn(async move {
                                if let Err(e) = services::icmp::stop_all_pings(&ah).await {
                                    log::error!("Failed to stop all pings from tray: {}", e);
                                }
                            });
                        }
                        "quit" => {
                            let ah = app.clone();
                            tauri::async_runtime::spawn(async move {
                                let _ = services::icmp::save_all_sessions(&ah).await;
                                let _ = services::bandwidth::save_bandwidth_session(&ah).await;
                                ah.exit(0);
                            });
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |_tray, event| {
                    // Double-click left mouse button: restore window
                    if let TrayIconEvent::DoubleClick {
                        button: MouseButton::Left,
                        ..
                    } = event
                    {
                        if let Some(window) = app_handle_for_tray.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            // Handle --minimized arg: hide window on startup
            let args: Vec<String> = std::env::args().collect();
            if args.iter().any(|a| a == "--minimized") {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.hide();
                }
            }

            // Wire up close-to-tray behaviour
            if let Some(window) = app.get_webview_window("main") {
                let window_clone = window.clone();
                let app_handle_for_close = app.handle().clone();
                let first_close = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));

                window.on_window_event(move |event| {
                    if let WindowEvent::CloseRequested { api, .. } = event {
                        if commands::settings::MINIMIZE_TO_TRAY.load(Ordering::Relaxed) {
                            api.prevent_close();
                            let _ = window_clone.hide();

                            // Show toast on first minimize
                            if first_close.swap(false, Ordering::Relaxed) {
                                let _ = app_handle_for_close.emit(
                                    "first-minimize",
                                    "应用已最小化到托盘，右键托盘图标可退出",
                                );
                            }
                        } else {
                            // Full quit: save sessions before letting Tauri close
                            let ah = app_handle_for_close.clone();
                            tauri::async_runtime::block_on(async move {
                                let _ = services::icmp::save_all_sessions(&ah).await;
                                let _ = services::bandwidth::save_bandwidth_session(&ah).await;
                            });
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping::start_ping,
            commands::ping::stop_ping,
            commands::ping::stop_all_pings,
            commands::ping::ping_once,
            commands::traceroute::run_traceroute,
            commands::traceroute::stop_traceroute,
            commands::continuous_trace::start_continuous_trace,
            commands::continuous_trace::stop_continuous_trace,
            commands::continuous_trace::list_trace_sessions,
            commands::continuous_trace::load_trace_hops,
            commands::continuous_trace::load_trace_samples,
            commands::continuous_trace::delete_trace_session,
            commands::continuous_trace::cleanup_old_trace_sessions,
            commands::bandwidth::start_bandwidth_test,
            commands::bandwidth::stop_bandwidth_test,
            commands::history::get_history,
            commands::history::save_ping_result,
            commands::history::clear_history,
            commands::network::dns_lookup,
            commands::network::get_network_info,
            commands::network::get_public_ip_info,
            commands::network::geoip_lookup,
            commands::network::geoip_lookup_batch,
            commands::settings::set_minimize_to_tray,
            commands::settings::is_autostart_enabled,
            commands::settings::set_autostart,
            commands::settings::quit_app,
            commands::settings::get_app_runtime_info,
            commands::alerts::trigger_webhook,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
