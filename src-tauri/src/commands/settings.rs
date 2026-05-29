use std::sync::atomic::Ordering;
use tauri_plugin_autostart::ManagerExt;

/// Atomic flag indicating whether the window should be hidden to the tray
/// when the user clicks the close button. Default: true (hide).
pub static MINIMIZE_TO_TRAY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(true);

#[tauri::command]
pub fn set_minimize_to_tray(enabled: bool) {
    MINIMIZE_TO_TRAY.store(enabled, Ordering::Relaxed);
}

#[tauri::command]
pub fn is_autostart_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_autostart(app: tauri::AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable().map_err(|e| e.to_string())
    } else {
        manager.disable().map_err(|e| e.to_string())
    }
}

/// Quit the application gracefully (saves pending sessions then exits).
#[tauri::command]
pub async fn quit_app(app: tauri::AppHandle) {
    if let Err(e) = crate::services::icmp::save_all_sessions(&app).await {
        log::error!("Failed to save ping sessions on quit: {}", e);
    }
    if let Err(e) = crate::services::bandwidth::save_bandwidth_session(&app).await {
        log::error!("Failed to save bandwidth session on quit: {}", e);
    }
    app.exit(0);
}
