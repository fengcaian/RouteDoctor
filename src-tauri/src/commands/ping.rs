use crate::models::ping::{PingConfig, PingResult};
use crate::services::icmp;
use crate::error::AppResult;

/// Start continuous ping to a target
#[tauri::command]
pub async fn start_ping(
    app_handle: tauri::AppHandle,
    target: String,
    interval_ms: u32,
    timeout_ms: u32,
    packet_size: u32,
) -> AppResult<()> {
    let config = PingConfig {
        target,
        interval_ms,
        timeout_ms,
        count: None,
        packet_size,
    };

    icmp::start_continuous_ping(app_handle, config).await
}

/// Stop ping for a target
#[tauri::command]
pub async fn stop_ping(app_handle: tauri::AppHandle, target: String) -> AppResult<()> {
    icmp::stop_ping(&app_handle, &target).await
}

/// Stop all active ping sessions
#[tauri::command]
pub async fn stop_all_pings(app_handle: tauri::AppHandle) -> AppResult<()> {
    icmp::stop_all_pings(&app_handle).await
}

/// Execute a single ping
#[tauri::command]
pub async fn ping_once(
    target: String,
    timeout_ms: u32,
    packet_size: u32,
) -> AppResult<PingResult> {
    icmp::ping_once(&target, timeout_ms, packet_size).await
}