use crate::services::bandwidth;
use crate::error::AppResult;

/// Start bandwidth test
#[tauri::command]
pub async fn start_bandwidth_test(
    app_handle: tauri::AppHandle,
) -> AppResult<()> {
    bandwidth::start_bandwidth_test(app_handle).await
}

/// Stop bandwidth test
#[tauri::command]
pub async fn stop_bandwidth_test(app_handle: tauri::AppHandle) -> AppResult<()> {
    bandwidth::stop_bandwidth_test(&app_handle).await
}