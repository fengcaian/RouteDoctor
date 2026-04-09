use crate::models::trace::{TracerouteConfig, ProbeMethod};
use crate::services::traceroute;
use crate::error::AppResult;

/// Run traceroute to a target
#[tauri::command]
pub async fn run_traceroute(
    app_handle: tauri::AppHandle,
    target: String,
    max_hops: u32,
    timeout_ms: u32,
    probe_method: ProbeMethod,
) -> AppResult<()> {
    let config = TracerouteConfig {
        target,
        max_hops,
        timeout_ms,
        probes_per_hop: 3,
        probe_method,
    };

    traceroute::run_traceroute(app_handle, config).await
}

/// Stop traceroute for a target
#[tauri::command]
pub async fn stop_traceroute(app_handle: tauri::AppHandle, target: String) -> AppResult<()> {
    traceroute::stop_traceroute(&app_handle, &target).await
}