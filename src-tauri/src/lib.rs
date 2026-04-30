// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod commands;
pub mod services;
pub mod models;
pub mod storage;
pub mod utils;
pub mod error;

use tauri::Listener;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logger for debug builds
    #[cfg(debug_assertions)]
    {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize database
            let app_handle = app.handle();
            storage::database::init_database(&app_handle)?;

            // Register window close handler to save pending data
            let app_handle = app.handle().clone();
            app.listen("tauri://window-close-requested", move |_| {
                let app_handle = app_handle.clone();
                tokio::spawn(async move {
                    log::info!("App closing, saving pending data...");

                    // Save pending ping sessions
                    if let Err(e) = services::icmp::save_all_sessions(&app_handle).await {
                        log::error!("Failed to save ping sessions: {}", e);
                    }

                    // Save pending bandwidth session
                    if let Err(e) = services::bandwidth::save_bandwidth_session(&app_handle).await {
                        log::error!("Failed to save bandwidth session: {}", e);
                    }

                    log::info!("All pending data saved, exiting...");
                });
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping::start_ping,
            commands::ping::stop_ping,
            commands::ping::stop_all_pings,
            commands::ping::ping_once,
            commands::traceroute::run_traceroute,
            commands::traceroute::stop_traceroute,
            commands::bandwidth::start_bandwidth_test,
            commands::bandwidth::stop_bandwidth_test,
            commands::history::get_history,
            commands::history::save_ping_result,
            commands::history::clear_history,
            commands::network::dns_lookup,
            commands::network::get_network_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}