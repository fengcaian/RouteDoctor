use std::sync::atomic::Ordering;
use tauri_plugin_autostart::ManagerExt;

/// 标记：点击窗口关闭按钮时是否隐藏到托盘。
/// 默认 false（直接退出应用）。前端 settings store 在启动时会通过
/// `set_minimize_to_tray` 把用户的偏好同步过来，覆盖此默认值。
pub static MINIMIZE_TO_TRAY: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

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

/// 应用运行时信息：用于"关于"页或调试展示数据存放位置和模式
#[derive(serde::Serialize)]
pub struct AppRuntimeInfo {
    /// 是否便携模式（exe 同目录有 portable.txt）
    pub portable: bool,
    /// 实际使用的数据目录绝对路径
    pub data_dir: String,
}

#[tauri::command]
pub fn get_app_runtime_info(app: tauri::AppHandle) -> Result<AppRuntimeInfo, String> {
    let portable = crate::storage::paths::is_portable_mode();
    let data_dir = crate::storage::paths::resolve_data_dir(&app)
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    Ok(AppRuntimeInfo { portable, data_dir })
}
