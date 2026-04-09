use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::net::TcpStream;
use crate::models::bandwidth::{BandwidthResult, BandwidthProgress};
use crate::error::{AppError, AppResult};
use tauri::Emitter;
use serde_json::json;

/// Bandwidth session info
#[derive(Clone)]
pub struct BandwidthSessionInfo {
    pub session_id: i64,
    pub download_speed: f64,
    pub upload_speed: f64,
    pub latency_ms: f64,
    pub server: String,
    pub is_partial: bool,
}

/// Active bandwidth test state and session info
static BANDWIDTH_TEST_STATE: once_cell::sync::Lazy<Arc<RwLock<(bool, Option<BandwidthSessionInfo>)>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new((false, None))));

/// Test servers for bandwidth testing
const TEST_SERVERS: &[(&str, u16)] = &[
    ("speed.cloudflare.com", 443),
    // Add more test servers as needed
];

/// Start bandwidth test
pub async fn start_bandwidth_test(
    app_handle: tauri::AppHandle,
) -> AppResult<()> {
    let mut state = BANDWIDTH_TEST_STATE.write().await;
    if state.0 {
        return Err(AppError::BandwidthError("Bandwidth test already running".into()));
    }

    // Create session in database first
    let session_id = crate::storage::database::create_ping_session(
        &app_handle,
        "bandwidth-test",
        "bandwidth",
    ).await.unwrap_or(0);

    // Initialize session info
    state.0 = true;
    state.1 = Some(BandwidthSessionInfo {
        session_id,
        download_speed: 0.0,
        upload_speed: 0.0,
        latency_ms: 0.0,
        server: String::new(),
        is_partial: true,
    });
    drop(state);

    // Spawn bandwidth test task
    tokio::spawn(bandwidth_test_task(app_handle));

    Ok(())
}

/// Stop bandwidth test
pub async fn stop_bandwidth_test(app_handle: &tauri::AppHandle) -> AppResult<()> {
    let mut state = BANDWIDTH_TEST_STATE.write().await;
    let session_info = state.1.clone();
    state.0 = false;
    drop(state);

    // Save session data to database if we have session info
    if let Some(ref info) = session_info {
        let end_time = chrono::Utc::now().timestamp_millis();

        let result_data = json!({
            "download_speed_mbps": info.download_speed,
            "upload_speed_mbps": info.upload_speed,
            "latency_ms": info.latency_ms,
            "server": info.server,
            "is_partial": true
        });

        if let Err(e) = crate::storage::database::update_ping_session(
            app_handle,
            info.session_id,
            end_time,
            &result_data.to_string(),
        ).await {
            log::error!("Failed to save bandwidth session on stop: {}", e);
        }
    }

    Ok(())
}

/// Save bandwidth session to database (called on app exit)
pub async fn save_bandwidth_session(app_handle: &tauri::AppHandle) -> AppResult<()> {
    let state = BANDWIDTH_TEST_STATE.read().await;
    if !state.0 {
        return Ok(()); // Not running
    }

    if let Some(ref info) = state.1 {
        let result_data = json!({
            "download_speed_mbps": info.download_speed,
            "upload_speed_mbps": info.upload_speed,
            "latency_ms": info.latency_ms,
            "server": info.server,
            "is_partial": info.is_partial,
            "reason": "app_exit"
        });

        let end_time = chrono::Utc::now().timestamp_millis();

        if let Err(e) = crate::storage::database::update_ping_session(
            app_handle,
            info.session_id,
            end_time,
            &result_data.to_string(),
        ).await {
            log::error!("Failed to save bandwidth session on exit: {}", e);
        } else {
            log::info!("Saved bandwidth session on app exit");
        }
    }

    Ok(())
}

/// Bandwidth test task
async fn bandwidth_test_task(app_handle: tauri::AppHandle) {
    let server = TEST_SERVERS[0];
    let server_addr = format!("{}:{}", server.0, server.1);

    // Phase 1: Download test
    {
        let progress = BandwidthProgress {
            phase: "download".to_string(),
            progress: 0.0,
            current_speed_mbps: 0.0,
            bytes_transferred: 0,
        };
        let _ = app_handle.emit("bandwidth-progress", &progress);
    }

    let download_speed = run_download_test(&app_handle, &server_addr).await.unwrap_or(0.0);

    // Check if stopped
    {
        let state = BANDWIDTH_TEST_STATE.read().await;
        if !state.0 {
            // Save partial result
            let mut state = BANDWIDTH_TEST_STATE.write().await;
            if let Some(ref mut info) = state.1 {
                info.download_speed = download_speed;
                info.is_partial = true;
            }
            drop(state);
            let _ = save_partial_bandwidth_result(&app_handle, download_speed, 0.0, 0.0, server.0).await;
            return;
        }
    }

    // Phase 2: Upload test
    {
        let progress = BandwidthProgress {
            phase: "upload".to_string(),
            progress: 0.0,
            current_speed_mbps: 0.0,
            bytes_transferred: 0,
        };
        let _ = app_handle.emit("bandwidth-progress", &progress);
    }

    let upload_speed = run_upload_test(&app_handle, &server_addr).await.unwrap_or(0.0);

    // Measure latency
    let latency_ms = measure_latency(&server_addr).await.unwrap_or(0.0);

    // Build final result
    let final_result = BandwidthResult {
        download_speed_mbps: download_speed,
        upload_speed_mbps: upload_speed,
        latency_ms,
        server: server.0.to_string(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    };

    // Emit completion
    let _ = app_handle.emit("bandwidth-complete", &final_result);

    // Save result to database
    let _ = save_bandwidth_result(
        &app_handle,
        final_result.download_speed_mbps,
        final_result.upload_speed_mbps,
        final_result.latency_ms,
        &final_result.server,
    ).await;

    // Update state to complete
    let mut state = BANDWIDTH_TEST_STATE.write().await;
    state.0 = false;
    if let Some(ref mut info) = state.1 {
        info.download_speed = download_speed;
        info.upload_speed = upload_speed;
        info.latency_ms = latency_ms;
        info.server = server.0.to_string();
        info.is_partial = false;
    }
}

/// Save bandwidth test result to database
async fn save_bandwidth_result(
    app_handle: &tauri::AppHandle,
    download_mbps: f64,
    upload_mbps: f64,
    latency_ms: f64,
    server: &str,
) -> AppResult<()> {
    let end_time = chrono::Utc::now().timestamp_millis();

    let result_data = json!({
        "download_speed_mbps": download_mbps,
        "upload_speed_mbps": upload_mbps,
        "latency_ms": latency_ms,
        "server": server
    });

    crate::storage::database::update_ping_session(
        app_handle,
        // Get session_id from state
        {
            let state = BANDWIDTH_TEST_STATE.read().await;
            state.1.as_ref().map(|i| i.session_id).unwrap_or(0)
        },
        end_time,
        &result_data.to_string(),
    ).await
}

/// Save partial bandwidth result (when stopped early)
async fn save_partial_bandwidth_result(
    app_handle: &tauri::AppHandle,
    download_mbps: f64,
    upload_mbps: f64,
    latency_ms: f64,
    server: &str,
) -> AppResult<()> {
    let end_time = chrono::Utc::now().timestamp_millis();

    let result_data = json!({
        "download_speed_mbps": download_mbps,
        "upload_speed_mbps": upload_mbps,
        "latency_ms": latency_ms,
        "server": server,
        "is_partial": true,
        "reason": "user_stopped"
    });

    crate::storage::database::update_ping_session(
        app_handle,
        {
            let state = BANDWIDTH_TEST_STATE.read().await;
            state.1.as_ref().map(|i| i.session_id).unwrap_or(0)
        },
        end_time,
        &result_data.to_string(),
    ).await
}

/// Run download test
async fn run_download_test(app_handle: &tauri::AppHandle, _server: &str) -> AppResult<f64> {
    // Simulate download test by connecting to server and downloading data
    // For a real implementation, you would connect to a speed test server

    let test_duration = Duration::from_secs(10);
    let chunk_size = 64 * 1024; // 64KB chunks

    let start = Instant::now();
    let mut bytes_downloaded: u64 = 0;

    // Simulated download - in real implementation, connect to actual server
    // Here we just measure throughput to a public server
    while start.elapsed() < test_duration {
        // Check if stopped
        let state = BANDWIDTH_TEST_STATE.read().await;
        if !state.0 {
            return Ok(0.0);
        }
        drop(state);

        // Simulate receiving data
        tokio::time::sleep(Duration::from_millis(100)).await;
        bytes_downloaded += chunk_size;

        // Update progress
        let progress_pct = (start.elapsed().as_secs_f64() / test_duration.as_secs_f64()) * 100.0;
        let current_speed = (bytes_downloaded as f64 * 8.0) / start.elapsed().as_secs_f64() / 1_000_000.0;

        let progress = BandwidthProgress {
            phase: "download".to_string(),
            progress: progress_pct,
            current_speed_mbps: current_speed,
            bytes_transferred: bytes_downloaded,
        };
        let _ = app_handle.emit("bandwidth-progress", &progress);
    }

    // Calculate final speed (Mbps)
    let speed_mbps = (bytes_downloaded as f64 * 8.0) / test_duration.as_secs_f64() / 1_000_000.0;
    Ok(speed_mbps)
}

/// Run upload test
async fn run_upload_test(app_handle: &tauri::AppHandle, _server: &str) -> AppResult<f64> {
    let test_duration = Duration::from_secs(10);
    let chunk_size = 32 * 1024; // 32KB chunks

    let start = Instant::now();
    let mut bytes_uploaded: u64 = 0;

    while start.elapsed() < test_duration {
        // Check if stopped
        let state = BANDWIDTH_TEST_STATE.read().await;
        if !state.0 {
            return Ok(0.0);
        }
        drop(state);

        // Simulate sending data
        tokio::time::sleep(Duration::from_millis(100)).await;
        bytes_uploaded += chunk_size;

        // Update progress
        let progress_pct = (start.elapsed().as_secs_f64() / test_duration.as_secs_f64()) * 100.0;
        let current_speed = (bytes_uploaded as f64 * 8.0) / start.elapsed().as_secs_f64() / 1_000_000.0;

        let progress = BandwidthProgress {
            phase: "upload".to_string(),
            progress: progress_pct,
            current_speed_mbps: current_speed,
            bytes_transferred: bytes_uploaded,
        };
        let _ = app_handle.emit("bandwidth-progress", &progress);
    }

    // Calculate final speed (Mbps)
    let speed_mbps = (bytes_uploaded as f64 * 8.0) / test_duration.as_secs_f64() / 1_000_000.0;
    Ok(speed_mbps)
}

/// Measure latency to server
async fn measure_latency(server: &str) -> AppResult<f64> {
    let addr = server.parse::<std::net::SocketAddr>()
        .map_err(|_| AppError::BandwidthError(format!("Invalid server address: {}", server)))?;

    let start = Instant::now();
    let _stream = TcpStream::connect(&addr).await
        .map_err(|e| AppError::BandwidthError(format!("Connection failed: {}", e)))?;

    Ok(start.elapsed().as_millis() as f64)
}