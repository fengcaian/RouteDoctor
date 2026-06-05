use crate::services::continuous_trace;
use crate::storage::trace_persist::{
    self, TraceSessionRow, TraceHopInfoRow, TraceSampleRow,
};
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
    persist: Option<bool>,
) -> AppResult<()> {
    continuous_trace::start_continuous_trace(
        app_handle,
        target,
        max_hops,
        timeout_ms,
        ping_interval_ms,
        probe_method.unwrap_or_else(|| "icmp".to_string()),
        persist.unwrap_or(true),
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

// ===== 持久化查询命令 =====

/// 列出最近的路径监控会话（默认 20 条）
#[tauri::command]
pub async fn list_trace_sessions(
    app_handle: tauri::AppHandle,
    limit: Option<u32>,
) -> AppResult<Vec<TraceSessionRow>> {
    trace_persist::list_sessions(&app_handle, limit.unwrap_or(20)).await
}

/// 加载某个会话的每跳元信息
#[tauri::command]
pub async fn load_trace_hops(
    app_handle: tauri::AppHandle,
    session_id: i64,
) -> AppResult<Vec<TraceHopInfoRow>> {
    trace_persist::load_hop_info(&app_handle, session_id).await
}

/// 加载某个会话的样本（since 为可选时间戳，limit 限制返回条数）
#[tauri::command]
pub async fn load_trace_samples(
    app_handle: tauri::AppHandle,
    session_id: i64,
    since: Option<i64>,
    limit: Option<u32>,
) -> AppResult<Vec<TraceSampleRow>> {
    trace_persist::load_samples(&app_handle, session_id, since, limit.unwrap_or(5000)).await
}

/// 删除某个会话
#[tauri::command]
pub async fn delete_trace_session(
    app_handle: tauri::AppHandle,
    session_id: i64,
) -> AppResult<()> {
    trace_persist::delete_session(&app_handle, session_id).await
}

/// 清理超过 N 天的会话（由 settings.maxHistoryDays 触发）
#[tauri::command]
pub async fn cleanup_old_trace_sessions(
    app_handle: tauri::AppHandle,
    days_to_keep: u32,
) -> AppResult<()> {
    trace_persist::cleanup_old_sessions(&app_handle, days_to_keep).await
}
