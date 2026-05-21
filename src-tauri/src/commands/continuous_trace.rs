use crate::services::continuous_trace;
use crate::error::AppResult;

/// 启动持续路径监控
/// 先跑一次 Traceroute 发现路径，然后对每一跳持续 Ping
#[tauri::command]
pub async fn start_continuous_trace(
    app_handle: tauri::AppHandle,
    target: String,
    max_hops: u32,
    timeout_ms: u32,
    ping_interval_ms: u32,
    probe_method: Option<String>,
) -> AppResult<()> {
    continuous_trace::start_continuous_trace(
        app_handle,
        target,
        max_hops,
        timeout_ms,
        ping_interval_ms,
        probe_method.unwrap_or_else(|| "icmp".to_string()),
    ).await
}

/// 停止持续路径监控
#[tauri::command]
pub async fn stop_continuous_trace(
    app_handle: tauri::AppHandle,
    target: String,
) -> AppResult<()> {
    continuous_trace::stop_continuous_trace(&app_handle, &target).await
}
