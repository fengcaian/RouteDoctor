use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::RwLock;
use crate::models::ping::{PingResult, PingConfig};
use crate::error::{AppError, AppResult};
use tauri::Emitter;
use serde_json::json;

/// Session info for tracking
pub struct PingSessionInfo {
    pub stop_tx: Sender<()>,
    pub session_id: i64,
    pub results: Vec<PingResult>,
}

// 限制每个 session 在内存中保留的结果数
const MAX_SESSION_RESULTS: usize = 1000;

/// Active ping sessions with their session IDs and results
static PING_SESSIONS: once_cell::sync::Lazy<Arc<RwLock<std::collections::HashMap<String, PingSessionInfo>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(std::collections::HashMap::new())));

/// Start continuous ping to a target
pub async fn start_continuous_ping(
    app_handle: tauri::AppHandle,
    config: PingConfig,
) -> AppResult<()> {
    let target = config.target.clone();

    // Check if already running
    let sessions = PING_SESSIONS.read().await;
    if sessions.contains_key(&target) {
        return Err(AppError::PingError("Ping already running for this target".into()));
    }
    drop(sessions);

    // Resolve target to IP
    let ip = resolve_target(&target).await?;

    // Create stop channel
    let (stop_tx, stop_rx) = mpsc::channel::<()>(1);

    // Create session in database FIRST to get session_id
    let session_id = crate::storage::database::create_ping_session(
        &app_handle,
        &target,
        "ping",
    ).await.unwrap_or(0);

    // Register session with session_id
    let mut sessions = PING_SESSIONS.write().await;
    sessions.insert(target.clone(), PingSessionInfo {
        stop_tx,
        session_id,
        results: Vec::new(),
    });
    drop(sessions);

    // Spawn ping loop
    tokio::spawn(ping_loop(app_handle, config, ip, stop_rx, session_id));

    Ok(())
}

/// Stop ping for a target
pub async fn stop_ping(app_handle: &tauri::AppHandle, target: &str) -> AppResult<()> {
    let mut sessions = PING_SESSIONS.write().await;
    if let Some(info) = sessions.remove(target) {
        let _ = info.stop_tx.send(()).await;

        // Save current session data to database
        let end_time = chrono::Utc::now().timestamp_millis();

        let results_json: Vec<serde_json::Value> = info.results.iter().map(|r| {
            json!({
                "seq": r.seq,
                "target": &r.target,
                "ip": &r.ip,
                "latency_ms": r.latency_ms,
                "is_timeout": r.is_timeout,
                "timestamp": r.timestamp
            })
        }).collect();

        let session_data = json!({
            "results": results_json,
            "statistics": calculate_session_statistics(&info.results)
        });

        drop(sessions);

        if let Err(e) = crate::storage::database::update_ping_session(
            app_handle,
            info.session_id,
            end_time,
            &session_data.to_string(),
        ).await {
            log::error!("Failed to save ping session on stop: {}", e);
        }
    }
    Ok(())
}

/// Stop all active ping sessions
pub async fn stop_all_pings(app_handle: &tauri::AppHandle) -> AppResult<()> {
    let mut sessions = PING_SESSIONS.write().await;
    let sessions_copy: Vec<_> = sessions.iter().map(|(t, i)| (t.clone(), i.session_id, i.stop_tx.clone(), i.results.clone())).collect();
    drop(sessions);

    for (_target, session_id, stop_tx, results) in sessions_copy {
        let _ = stop_tx.send(()).await;

        let end_time = chrono::Utc::now().timestamp_millis();

        let results_json: Vec<serde_json::Value> = results.iter().map(|r| {
            json!({
                "seq": r.seq,
                "target": &r.target,
                "ip": &r.ip,
                "latency_ms": r.latency_ms,
                "is_timeout": r.is_timeout,
                "timestamp": r.timestamp
            })
        }).collect();

        let session_data = json!({
            "results": results_json,
            "statistics": calculate_session_statistics(&results)
        });

        if let Err(e) = crate::storage::database::update_ping_session(
            app_handle,
            session_id,
            end_time,
            &session_data.to_string(),
        ).await {
            log::error!("Failed to save ping session on stop all: {}", e);
        }
    }
    Ok(())
}

/// Save all active ping sessions to database (called on app exit)
pub async fn save_all_sessions(app_handle: &tauri::AppHandle) -> AppResult<()> {
    let mut sessions = PING_SESSIONS.write().await;

    for (target, info) in sessions.drain() {
        let end_time = chrono::Utc::now().timestamp_millis();

        let results_json: Vec<serde_json::Value> = info.results.iter().map(|r| {
            json!({
                "seq": r.seq,
                "target": &r.target,
                "ip": &r.ip,
                "latency_ms": r.latency_ms,
                "is_timeout": r.is_timeout,
                "timestamp": r.timestamp
            })
        }).collect();

        let session_data = json!({
            "results": results_json,
            "statistics": calculate_session_statistics(&info.results),
            "reason": "app_exit"
        });

        if let Err(e) = crate::storage::database::update_ping_session(
            app_handle,
            info.session_id,
            end_time,
            &session_data.to_string(),
        ).await {
            log::error!("Failed to save ping session for {} on exit: {}", target, e);
        } else {
            log::info!("Saved ping session for {} on app exit", target);
        }
    }

    Ok(())
}

/// Single ping operation using system ping command
pub async fn ping_once(target: &str, timeout_ms: u32, packet_size: u32) -> AppResult<PingResult> {
    let ip = resolve_target(target).await?;

    // Use Windows ping command with timeout and packet size
    let output = tokio::process::Command::new("ping")
        .args([
            "-n", "1",
            "-w", &timeout_ms.to_string(),
            "-l", &packet_size.to_string(),
            &ip.to_string()
        ])
        .output()
        .await
        .map_err(|e| AppError::PingError(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Parse latency from output
    // Example: "Reply from 180.101.49.12: bytes=32 time=24ms TTL=50"
    let latency_ms = parse_ping_time(&stdout);

    Ok(PingResult {
        seq: 0,
        target: target.to_string(),
        ip: ip.to_string(),
        latency_ms,
        is_timeout: latency_ms.is_none(),
        timestamp: chrono::Utc::now().timestamp_millis(),
    })
}

/// Parse time=XXms from ping output
/// Uses byte-level scanning to work with any encoding (UTF-8, GBK, etc.)
fn parse_ping_time(output: &str) -> Option<f64> {
    let bytes = output.as_bytes();
    let len = bytes.len();

    // First check for "<1ms" pattern (sub-millisecond response)
    // Works for both "time<1ms" and "时间<1ms" in any encoding
    for i in 0..len.saturating_sub(3) {
        if bytes[i] == b'<'
            && bytes[i + 1].is_ascii_digit()
            && bytes[i + 2] == b'm'
            && bytes[i + 3] == b's'
        {
            return Some(0.5);
        }
    }

    // Scan for pattern: =<number>ms
    // This works for both "time=7ms" (English) and "时间=7ms" (Chinese)
    // regardless of text encoding, since '=', digits, and "ms" are all ASCII
    let mut i = 0;
    while i < len {
        if bytes[i] == b'=' {
            let start = i + 1;
            let mut j = start;

            // Read digits and optional decimal point
            while j < len && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
                j += 1;
            }

            // Check if digits are immediately followed by "ms"
            if j > start && j + 1 < len && bytes[j] == b'm' && bytes[j + 1] == b's' {
                if let Ok(val) = output[start..j].parse::<f64>() {
                    return Some(val);
                }
            }
        }
        i += 1;
    }
    None
}

/// Ping loop task using system ping command
async fn ping_loop(
    app_handle: tauri::AppHandle,
    config: PingConfig,
    ip: IpAddr,
    mut stop_rx: Receiver<()>,
    session_id: i64,
) {
    let interval = Duration::from_millis(config.interval_ms as u64);
    let timeout = config.timeout_ms as u64;

    let mut seq: u32 = 0;
    let max_count = config.count.unwrap_or(u32::MAX);

    loop {
        // Check for stop signal
        if stop_rx.try_recv() != Err(mpsc::error::TryRecvError::Empty) {
            break;
        }

        if seq >= max_count {
            break;
        }

        // Use Windows ping command with configured packet size
        let output = tokio::process::Command::new("ping")
            .args([
                "-n", "1",
                "-l", &config.packet_size.to_string(), // Set packet size
                "-w", &timeout.to_string(),
                &ip.to_string()
            ])
            .output()
            .await;

        let ping_result = match output {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let latency_ms = parse_ping_time(&stdout);

                PingResult {
                    seq,
                    target: config.target.clone(),
                    ip: ip.to_string(),
                    latency_ms,
                    is_timeout: latency_ms.is_none(),
                    timestamp: chrono::Utc::now().timestamp_millis(),
                }
            }
            Err(e) => {
                log::error!("Ping command error: {}", e);
                PingResult {
                    seq,
                    target: config.target.clone(),
                    ip: ip.to_string(),
                    latency_ms: None,
                    is_timeout: true,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                }
            }
        };

        // Store result in global session state (with size limit)
        {
            let mut sessions = PING_SESSIONS.write().await;
            if let Some(info) = sessions.get_mut(&config.target) {
                info.results.push(ping_result.clone());
                // Keep only last MAX_SESSION_RESULTS to prevent memory leak
                if info.results.len() > MAX_SESSION_RESULTS {
                    info.results.remove(0);
                }
            }
        }

        // Emit result to frontend
        if let Err(e) = app_handle.emit("ping-result", &ping_result) {
            log::error!("Failed to emit ping result: {}", e);
        }

        seq += 1;

        // Wait for interval
        tokio::time::sleep(interval).await;
    }

    // Save session result to database
    let end_time = chrono::Utc::now().timestamp_millis();
    let sessions = PING_SESSIONS.read().await;
    if let Some(info) = sessions.get(&config.target) {
        let results_json: Vec<serde_json::Value> = info.results.iter().map(|r| {
            json!({
                "seq": r.seq,
                "target": &r.target,
                "ip": &r.ip,
                "latency_ms": r.latency_ms,
                "is_timeout": r.is_timeout,
                "timestamp": r.timestamp
            })
        }).collect();

        let session_data = json!({
            "results": results_json,
            "statistics": calculate_session_statistics(&info.results)
        });

        drop(sessions);
        if let Err(e) = crate::storage::database::update_ping_session(
            &app_handle,
            session_id,
            end_time,
            &session_data.to_string(),
        ).await {
            log::error!("Failed to save ping session: {}", e);
        }
    }
}

/// Calculate statistics for a ping session
fn calculate_session_statistics(results: &[PingResult]) -> serde_json::Value {
    if results.is_empty() {
        return json!({});
    }

    let sent = results.len() as i32;
    let timeouts = results.iter().filter(|r| r.is_timeout).count() as i32;
    let received = sent - timeouts;
    let latencies: Vec<f64> = results
        .iter()
        .filter(|r| !r.is_timeout && r.latency_ms.is_some())
        .map(|r| r.latency_ms.unwrap())
        .collect();

    let loss_rate = if sent > 0 { (timeouts as f64 / sent as f64) * 100.0 } else { 0.0 };

    let (min_ms, max_ms, avg_ms, std_dev_ms, jitter_ms) = if latencies.is_empty() {
        (0.0, 0.0, 0.0, 0.0, 0.0)
    } else {
        let min_ms = latencies.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_ms = latencies.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let avg_ms = latencies.iter().sum::<f64>() / latencies.len() as f64;

        let variance = latencies.iter().map(|l| (l - avg_ms).powi(2)).sum::<f64>() / latencies.len() as f64;
        let std_dev_ms = variance.sqrt();

        let mut jitter_sum = 0.0;
        for i in 1..latencies.len() {
            jitter_sum += (latencies[i] - latencies[i - 1]).abs();
        }
        let jitter_ms = if latencies.len() > 1 { jitter_sum / (latencies.len() - 1) as f64 } else { 0.0 };

        (min_ms, max_ms, avg_ms, std_dev_ms, jitter_ms)
    };

    json!({
        "sent": sent,
        "received": received,
        "lost": timeouts,
        "loss_rate": loss_rate,
        "min_ms": min_ms,
        "max_ms": max_ms,
        "avg_ms": avg_ms,
        "std_dev_ms": std_dev_ms,
        "jitter_ms": jitter_ms
    })
}

/// Resolve target hostname/IP to IP address (async version)
/// Uses system DNS configuration for accurate local resolution (e.g. CDN-friendly),
/// falls back to Google DNS (8.8.8.8) if system config is unavailable.
async fn resolve_target(target: &str) -> AppResult<IpAddr> {
    if let Ok(ip) = target.parse::<IpAddr>() {
        return Ok(ip);
    }

    use trust_dns_resolver::TokioAsyncResolver;
    use trust_dns_resolver::config::*;

    let resolver = match TokioAsyncResolver::tokio_from_system_conf() {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to load system DNS config: {}, falling back to public DNS", e);
            // Fallback to multiple public DNS providers for global coverage:
            // - Cloudflare (1.1.1.1) - fast worldwide
            // - Google (8.8.8.8) - reliable worldwide
            // - AliDNS (223.5.5.5) - optimized for China
            let ips: Vec<std::net::IpAddr> = vec![
                "1.1.1.1".parse().unwrap(),
                "8.8.8.8".parse().unwrap(),
                "223.5.5.5".parse().unwrap(),
            ];
            let config = ResolverConfig::from_parts(
                None,
                vec![],
                NameServerConfigGroup::from_ips_clear(&ips, 53, true),
            );
            TokioAsyncResolver::tokio(config, ResolverOpts::default())
        }
    };

    let response = resolver.lookup_ip(target)
        .await
        .map_err(|e| AppError::DnsError(e.to_string()))?;

    match response.iter().next() {
        Some(ip) => Ok(ip),
        None => Err(AppError::DnsError(format!("Could not resolve {}", target))),
    }
}

/// Check if ping is running for a target
pub async fn is_ping_running(target: &str) -> bool {
    let sessions = PING_SESSIONS.read().await;
    sessions.contains_key(target)
}
