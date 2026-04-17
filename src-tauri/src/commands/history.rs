use crate::models::ping::PingResult;
use crate::storage::database;
use crate::error::AppResult;
use serde::{Deserialize, Serialize};

/// History record
#[derive(Debug, Serialize, Deserialize)]
pub struct HistoryRecord {
    pub id: i64,
    pub target: String,
    pub test_type: String,
    pub start_time: i64,
    pub end_time: i64,
    pub data: String,
}

/// Get history records for a target
#[tauri::command]
pub async fn get_history(
    app_handle: tauri::AppHandle,
    target: Option<String>,
    test_type: Option<String>,
    limit: Option<u32>,
) -> AppResult<Vec<HistoryRecord>> {
    database::get_history(&app_handle, target, test_type, limit).await
}

/// Save a ping result to history
#[tauri::command]
pub async fn save_ping_result(
    app_handle: tauri::AppHandle,
    result: PingResult,
) -> AppResult<()> {
    database::save_ping_result(&app_handle, result).await
}

/// 清除所有历史记录
#[tauri::command]
pub async fn clear_history(
    app_handle: tauri::AppHandle,
) -> AppResult<()> {
    database::clear_all_history(&app_handle).await
}
