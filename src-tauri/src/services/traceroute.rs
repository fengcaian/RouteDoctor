use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::RwLock;
use crate::models::trace::{TracerouteResult, HopResult, TracerouteConfig, ProbeMethod};
use crate::services::dns::{resolve, reverse_lookup};
use crate::error::{AppError, AppResult};
use tauri::Emitter;
use serde_json::json;

/// Session info for tracking
pub struct TraceSessionInfo {
    pub stop_tx: Sender<()>,
    pub session_id: i64,
    pub result: TracerouteResult,
}

/// Active traceroute sessions
static TRACE_SESSIONS: once_cell::sync::Lazy<Arc<RwLock<std::collections::HashMap<String, TraceSessionInfo>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(std::collections::HashMap::new())));

/// Run traceroute to a target
pub async fn run_traceroute(
    app_handle: tauri::AppHandle,
    config: TracerouteConfig,
) -> AppResult<()> {
    let target = config.target.clone();

    // Check if already running
    let sessions = TRACE_SESSIONS.read().await;
    if sessions.contains_key(&target) {
        return Err(AppError::TracerouteError("Traceroute already running for this target".into()));
    }
    drop(sessions);

    // Resolve target to IP
    let target_ip = resolve(&target).await?;

    // Create stop channel
    let (stop_tx, stop_rx) = mpsc::channel::<()>(1);

    // Create session in database to get session_id
    let session_id = crate::storage::database::create_ping_session(
        &app_handle,
        &target,
        "traceroute",
    ).await.unwrap_or(0);

    // Create initial result
    let start_time = chrono::Utc::now().timestamp_millis();
    let result = TracerouteResult {
        target: target.clone(),
        target_ip: target_ip.to_string(),
        hops: Vec::new(),
        completed: false,
        start_time,
        end_time: None,
        probe_method: config.probe_method.clone(),
    };

    // Register session with session_id and result
    let mut sessions = TRACE_SESSIONS.write().await;
    sessions.insert(target.clone(), TraceSessionInfo {
        stop_tx,
        session_id,
        result,
    });
    drop(sessions);

    // Spawn traceroute task based on probe method
    tokio::spawn(traceroute_task(app_handle, config, target_ip, stop_rx));

    Ok(())
}

/// Stop traceroute for a target
pub async fn stop_traceroute(app_handle: &tauri::AppHandle, target: &str) -> AppResult<()> {
    let mut sessions = TRACE_SESSIONS.write().await;
    if let Some(info) = sessions.remove(target) {
        let _ = info.stop_tx.send(()).await;

        // Save current session data to database
        let end_time = chrono::Utc::now().timestamp_millis();

        // Build result data for storage
        let hops_data: Vec<serde_json::Value> = info.result.hops.iter().map(|hop| {
            json!({
                "hop_number": hop.hop_number,
                "ip": hop.ip,
                "hostname": hop.hostname,
                "latencies": hop.latencies,
                "avg_latency": hop.avg_latency,
                "packet_loss": hop.packet_loss
            })
        }).collect();

        let result_data = json!({
            "target": info.result.target,
            "target_ip": info.result.target_ip,
            "hops": hops_data,
            "completed": info.result.completed,
            "probe_method": info.result.probe_method.to_string()
        });

        drop(sessions);

        // Save to database
        if let Err(e) = crate::storage::database::update_ping_session(
            app_handle,
            info.session_id,
            end_time,
            &result_data.to_string(),
        ).await {
            log::error!("Failed to save traceroute session on stop: {}", e);
        }
    }
    Ok(())
}

/// Traceroute task - dispatches to appropriate implementation
async fn traceroute_task(
    app_handle: tauri::AppHandle,
    config: TracerouteConfig,
    target_ip: IpAddr,
    stop_rx: Receiver<()>,
) {
    match config.probe_method {
        ProbeMethod::Icmp => run_icmp_traceroute(app_handle, config, target_ip, stop_rx).await,
        ProbeMethod::Udp => run_udp_traceroute(app_handle, config, target_ip, stop_rx).await,
        ProbeMethod::Tcp => run_tcp_traceroute(app_handle, config, target_ip, stop_rx).await,
    }
}

/// ICMP traceroute - uses system tracert command (Windows) or traceroute -I (Unix)
async fn run_icmp_traceroute(
    app_handle: tauri::AppHandle,
    config: TracerouteConfig,
    target_ip: IpAddr,
    mut stop_rx: Receiver<()>,
) {
    let start_time = chrono::Utc::now().timestamp_millis();
    let probe_method = ProbeMethod::Icmp;

    // Create session in database
    let session_id = crate::storage::database::create_ping_session(
        &app_handle,
        &config.target,
        "traceroute",
    ).await.unwrap_or(0);

    // For Windows, use tracert command (ICMP by default)
    // For Unix, use traceroute -I for ICMP
    let (command, base_args) = if cfg!(windows) {
        ("tracert", vec![
            "-d".to_string(), // Don't resolve hostnames
            "-h".to_string(), config.max_hops.to_string(),
            "-w".to_string(), config.timeout_ms.to_string(),
        ])
    } else {
        ("traceroute", vec![
            "-I".to_string(), // Use ICMP
            "-n".to_string(), // Don't resolve hostnames
            "-m".to_string(), config.max_hops.to_string(),
            "-w".to_string(), format!("{}", config.timeout_ms / 1000), // seconds on Unix
        ])
    };

    let args = [base_args, vec![config.target.clone()]].concat();
    log::info!("ICMP traceroute to {} with args: {:?}", config.target, args);

    let mut result = TracerouteResult {
        target: config.target.clone(),
        target_ip: target_ip.to_string(),
        hops: Vec::new(),
        completed: false,
        start_time,
        end_time: None,
        probe_method,
    };

    let mut cmd = tokio::process::Command::new(command);
    cmd.args(&args);
    cmd.kill_on_drop(true);
    cmd.stdin(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());

    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    match cmd.spawn() {
        Ok(mut child) => {
            // Read stdout line by line in real-time
            use tokio::io::{AsyncBufReadExt, BufReader};
            let stdout = child.stdout.take().expect("stdout not available");
            let mut lines = BufReader::new(stdout).lines();

            while let Ok(Some(line)) = lines.next_line().await {
                // Check for stop signal
                if stop_rx.try_recv() != Err(mpsc::error::TryRecvError::Empty) {
                    child.kill().await.ok();
                    break;
                }

                if let Some(hop) = parse_traceroute_line(&line) {
                    process_hop(&app_handle, &config.target, hop, &mut result).await;
                }
            }

            // Wait for child to complete
            let _ = child.wait().await;
            result.completed = true;
        }
        Err(e) => {
            log::error!("ICMP traceroute failed: {}", e);
            result.completed = false;
            let _ = app_handle.emit("trace-error", &format!("ICMP Traceroute failed: {}", e));
        }
    }

    finalize_result(&app_handle, result, &config.target, session_id).await;
}

/// UDP traceroute - 优先用并行 UDP；失败时用并行 ICMP（与 Windows tracert 等价但快得多）
async fn run_udp_traceroute(
    app_handle: tauri::AppHandle,
    config: TracerouteConfig,
    target_ip: IpAddr,
    stop_rx: Receiver<()>,
) {
    // 仅 IPv4 支持快速并行实现；IPv6 直接回退
    if let IpAddr::V4(ipv4) = target_ip {
        log::info!("Trying parallel UDP traceroute to {} ({})", config.target, ipv4);
        let fast = crate::services::fast_udp_traceroute::parallel_udp_traceroute(
            ipv4, config.max_hops, config.timeout_ms,
        ).await;

        let fast_hops = match fast {
            Ok(hops) if hops.iter().any(|h| h.ip.is_some()) => Some(hops),
            Ok(_) => {
                log::warn!("Parallel UDP returned no hops, will try parallel ICMP fallback");
                None
            }
            Err(e) => {
                log::warn!("Parallel UDP failed ({}), will try parallel ICMP fallback", e);
                None
            }
        };

        // UDP fast 失败时优先走并行 ICMP（仍是 raw socket，速度同级）
        let final_hops = if let Some(h) = fast_hops {
            Some(h)
        } else {
            log::info!("Trying parallel ICMP fallback for UDP traceroute to {}", config.target);
            match crate::services::fast_traceroute::parallel_icmp_traceroute(
                ipv4, config.max_hops, config.timeout_ms,
            ).await {
                Ok(h) if h.iter().any(|x| x.ip.is_some()) => Some(h),
                _ => None,
            }
        };

        if let Some(fast_hops) = final_hops {
            let start_time = chrono::Utc::now().timestamp_millis();
            let session_id = crate::storage::database::create_ping_session(
                &app_handle,
                &config.target,
                "traceroute",
            ).await.unwrap_or(0);

            let mut result = TracerouteResult {
                target: config.target.clone(),
                target_ip: target_ip.to_string(),
                hops: Vec::new(),
                completed: true,
                start_time,
                end_time: None,
                probe_method: ProbeMethod::Udp,
            };
            for fh in fast_hops {
                let hop = HopResult {
                    hop_number: fh.hop_number,
                    ip: fh.ip,
                    hostname: None,
                    latencies: vec![fh.rtt_ms],
                    avg_latency: fh.rtt_ms,
                    packet_loss: if fh.rtt_ms.is_some() { 0.0 } else { 100.0 },
                };
                process_hop(&app_handle, &config.target, hop, &mut result).await;
            }
            finalize_result(&app_handle, result, &config.target, session_id).await;
            return;
        }
    }

    // 最终回退：raw socket 完全不可用时走系统 ICMP 命令
    log::warn!("All parallel implementations failed, falling back to system ICMP command");
    run_icmp_traceroute(app_handle, config, target_ip, stop_rx).await
}

/// TCP traceroute - 优先用并行 TCP；失败时用并行 ICMP（路径与 TCP SYN 通常一致）
async fn run_tcp_traceroute(
    app_handle: tauri::AppHandle,
    config: TracerouteConfig,
    target_ip: IpAddr,
    mut stop_rx: Receiver<()>,
) {
    let start_time = chrono::Utc::now().timestamp_millis();
    let probe_method = ProbeMethod::Tcp;

    // 解析目标端口
    let (_target_host, target_port) = parse_target_with_port(&config.target);

    // 优先尝试并行 TCP traceroute（仅 IPv4），失败时再试并行 ICMP
    if let IpAddr::V4(ipv4) = target_ip {
        log::info!("Trying parallel TCP traceroute to {}:{} ({})", config.target, target_port, ipv4);
        let fast = crate::services::fast_tcp_traceroute::parallel_tcp_traceroute(
            ipv4, target_port, config.max_hops, config.timeout_ms,
        ).await;

        let fast_hops = match fast {
            Ok(hops) if hops.iter().any(|h| h.ip.is_some()) => Some(hops),
            Ok(_) => {
                log::warn!("Parallel TCP returned no hops, will try parallel ICMP fallback");
                None
            }
            Err(e) => {
                log::warn!("Parallel TCP failed ({}), will try parallel ICMP fallback", e);
                None
            }
        };

        let final_hops = if let Some(h) = fast_hops {
            Some(h)
        } else {
            log::info!("Trying parallel ICMP fallback for TCP traceroute to {}", config.target);
            match crate::services::fast_traceroute::parallel_icmp_traceroute(
                ipv4, config.max_hops, config.timeout_ms,
            ).await {
                Ok(h) if h.iter().any(|x| x.ip.is_some()) => Some(h),
                _ => None,
            }
        };

        if let Some(fast_hops) = final_hops {
            let session_id = crate::storage::database::create_ping_session(
                &app_handle,
                &config.target,
                "traceroute",
            ).await.unwrap_or(0);

            let mut result = TracerouteResult {
                target: config.target.clone(),
                target_ip: target_ip.to_string(),
                hops: Vec::new(),
                completed: true,
                start_time,
                end_time: None,
                probe_method,
            };
            for fh in fast_hops {
                let hop = HopResult {
                    hop_number: fh.hop_number,
                    ip: fh.ip,
                    hostname: None,
                    latencies: vec![fh.rtt_ms],
                    avg_latency: fh.rtt_ms,
                    packet_loss: if fh.rtt_ms.is_some() { 0.0 } else { 100.0 },
                };
                process_hop(&app_handle, &config.target, hop, &mut result).await;
            }
            finalize_result(&app_handle, result, &config.target, session_id).await;
            return;
        }
    }

    // 最终回退：使用 tracetcp（Windows）/ traceroute -T（Unix）系统命令
    log::warn!("All parallel implementations failed, falling back to system tracetcp");
    let session_id = crate::storage::database::create_ping_session(
        &app_handle,
        &config.target,
        "traceroute",
    ).await.unwrap_or(0);

    // Use tracetcp command on Windows for TCP traceroute
    let (command, args) = if cfg!(windows) {
        let (target_host, port) = parse_target_with_port(&config.target);
        let target_with_port = format!("{}:{}", target_host, port);

        let args = vec![
            target_with_port,
            "-m".to_string(), config.max_hops.to_string(),
            "-t".to_string(), config.timeout_ms.to_string(),
            "-n".to_string(),
            "-p".to_string(), "3".to_string(),
        ];

        ("tracetcp", args)
    } else {
        let base_args = vec![
            "-T".to_string(),
            "-n".to_string(),
            "-m".to_string(), config.max_hops.to_string(),
            "-w".to_string(), format!("{}", config.timeout_ms / 1000),
            config.target.clone(),
        ];
        ("traceroute", base_args)
    };

    log::info!("TCP traceroute (system) to {} with args: {:?}", config.target, args);

    let mut result = TracerouteResult {
        target: config.target.clone(),
        target_ip: target_ip.to_string(),
        hops: Vec::new(),
        completed: false,
        start_time,
        end_time: None,
        probe_method,
    };

    let mut cmd = tokio::process::Command::new(command);
    cmd.args(&args);
    cmd.kill_on_drop(true);
    cmd.stdin(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::piped());
    cmd.stdout(std::process::Stdio::piped());

    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000);
    }

    match cmd.spawn() {
        Ok(mut child) => {
            // Read stdout line by line in real-time
            use tokio::io::{AsyncBufReadExt, BufReader};
            let stdout = child.stdout.take().expect("stdout not available");
            let mut lines = BufReader::new(stdout).lines();

            while let Ok(Some(line)) = lines.next_line().await {
                // Check for stop signal
                if stop_rx.try_recv() != Err(mpsc::error::TryRecvError::Empty) {
                    child.kill().await.ok();
                    break;
                }

                if let Some(hop) = parse_tracetcp_line(&line) {
                    process_hop(&app_handle, &config.target, hop, &mut result).await;
                }
            }

            // Wait for child to complete
            let _ = child.wait().await;
            result.completed = true;
        }
        Err(e) => {
            log::error!("TCP traceroute failed: {}", e);
            result.completed = false;
            let _ = app_handle.emit("trace-error", &format!("TCP Traceroute failed: {}", e));
        }
    }

    finalize_result(&app_handle, result, &config.target, session_id).await;
}

/// Parse target with port (e.g., "google.com:443" -> ("google.com", 443))
fn parse_target_with_port(target: &str) -> (String, u16) {
    if let Some(colon_pos) = target.rfind(':') {
        // Check if the part after colon is a port number (not IPv6)
        let after_colon = &target[colon_pos + 1..];
        if let Ok(port) = after_colon.parse::<u16>() {
            return (target[..colon_pos].to_string(), port);
        }
    }
    // Default port for HTTPS
    (target.to_string(), 443)
}

/// Process a hop result and emit to frontend
async fn process_hop(
    app_handle: &tauri::AppHandle,
    target: &str,
    hop: HopResult,
    result: &mut TracerouteResult,
) {
    // Resolve hostname and GeoIP in parallel
    if let Some(ref ip_str) = hop.ip {
        if let Ok(ip) = ip_str.parse::<IpAddr>() {
            let (hostname_res, geo_res) = tokio::join!(
                reverse_lookup(&ip),
                crate::services::geoip::lookup_one(&ip)
            );

            let hostname = hostname_res.ok().flatten();

            // Emit geo info to frontend separately (so the hop event is not delayed)
            if let Some(geo) = geo_res {
                let _ = app_handle.emit("trace-hop-geo", serde_json::json!({
                    "target": target,
                    "hop_number": hop.hop_number,
                    "ip": ip_str,
                    "geo": geo,
                }));
            }

            let hop_with_hostname = HopResult {
                hostname,
                ..hop.clone()
            };
            result.hops.push(hop_with_hostname.clone());
            emit_hop(app_handle, target, &hop_with_hostname);
            return;
        }
    }

    result.hops.push(hop.clone());
    emit_hop(app_handle, target, &hop);
}

/// Finalize traceroute result and emit completion
async fn finalize_result(
    app_handle: &tauri::AppHandle,
    result: TracerouteResult,
    target: &str,
    session_id: i64,
) {
    let end_time = chrono::Utc::now().timestamp_millis();

    // Build result data for storage
    let hops_data: Vec<serde_json::Value> = result.hops.iter().map(|hop| {
        json!({
            "hop_number": hop.hop_number,
            "ip": hop.ip,
            "hostname": hop.hostname,
            "latencies": hop.latencies,
            "avg_latency": hop.avg_latency,
            "packet_loss": hop.packet_loss
        })
    }).collect();

    let result_data = json!({
        "target": result.target,
        "target_ip": result.target_ip,
        "hops": hops_data,
        "completed": result.completed,
        "probe_method": result.probe_method.to_string()
    });

    // Save to database
    if let Err(e) = crate::storage::database::update_ping_session(
        app_handle,
        session_id,
        end_time,
        &result_data.to_string(),
    ).await {
        log::error!("Failed to save traceroute session: {}", e);
    }

    let final_result = TracerouteResult {
        end_time: Some(end_time),
        ..result
    };

    if let Err(e) = app_handle.emit("trace-complete", &final_result) {
        log::error!("Failed to emit trace complete: {}", e);
    }

    // Cleanup session - convert target to owned string
    let target_owned = target.to_string();
    tokio::spawn(async move {
        let mut sessions = TRACE_SESSIONS.write().await;
        sessions.remove(&target_owned);
    });
}

/// Calculate average latency from probe results
fn calculate_avg_latency(latencies: &[Option<f64>]) -> Option<f64> {
    let valid: Vec<f64> = latencies.iter().filter_map(|l| *l).collect();
    if valid.is_empty() {
        None
    } else {
        Some(valid.iter().sum::<f64>() / valid.len() as f64)
    }
}

/// Calculate packet loss percentage
fn calculate_packet_loss(latencies: &[Option<f64>]) -> f64 {
    if latencies.is_empty() {
        return 0.0;
    }
    let timeouts = latencies.iter().filter(|l| l.is_none()).count();
    (timeouts as f64 / latencies.len() as f64) * 100.0
}

/// Parse a tracetcp output line
/// tracetcp format:
///  1       4 ms    4 ms    4 ms    172.20.167.1
///  2       2 ms    7 ms    2 ms    172.20.255.17
///  3       2 ms    2 ms    1 ms    192.168.193.254
///  4       Destination Reached in 9 ms. Connection established to 172.18.13.75
fn parse_tracetcp_line(line: &str) -> Option<HopResult> {
    // Skip empty lines and header/footer lines
    let line_lower = line.to_lowercase();
    if line.trim().is_empty()
        || line_lower.contains("tracing route")
        || line_lower.contains("over a maximum")
        || line_lower.contains("trace complete")
        || line_lower.contains("traceroute to")
        || line_lower.contains("hops max")
        || line_lower.contains("tracing tcp")
        || line_lower.contains("on port")
    {
        return None;
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    // First part should be hop number
    let hop_number: u32 = parts[0].parse().ok()?;

    // Skip invalid hop numbers
    if hop_number == 0 || hop_number > 100 {
        return None;
    }

    let mut ip: Option<String> = None;
    let mut latencies: Vec<Option<f64>> = Vec::new();

    // Check for "Destination Reached" line (final hop)
    let dest_reached_idx = parts.iter().position(|&p| p == "Destination");
    if let Some(idx) = dest_reached_idx {
        // Parse: "Destination Reached in 9 ms. Connection established to 172.18.13.75"
        // Look for "in X ms" pattern
        for i in (idx + 3)..parts.len() {
            // Check for "ms" or "ms." (with trailing punctuation)
            let part_clean = parts[i].trim_end_matches(|c: char| c == '.' || c == ',');
            if part_clean == "ms" && i > idx + 3 {
                if let Ok(latency) = parts[i - 1].parse::<f64>() {
                    latencies.push(Some(latency));
                }
                break;
            }
        }
        // Look for IP at the end
        for part in parts.iter().rev() {
            if part.contains('.') && part.parse::<IpAddr>().is_ok() {
                ip = Some(part.to_string());
                break;
            }
        }

        if !latencies.is_empty() || ip.is_some() {
            let avg_latency = latencies.first().copied().flatten();
            return Some(HopResult {
                hop_number,
                ip,
                hostname: None,
                latencies,
                avg_latency,
                packet_loss: 0.0,
            });
        }
        return None;
    }

    // Standard format: hop_number  latency  latency  latency  IP
    // Example: 1       4 ms    4 ms    4 ms    172.20.167.1
    let mut i = 1;

    while i < parts.len() {
        let part = parts[i];

        // Check for timeout (shouldn't happen in tracetcp, but handle it)
        if part == "*" {
            latencies.push(None);
            i += 1;
            continue;
        }

        // Check for "Request" (part of "Request timed out")
        if part == "Request" || part == "timed" || part == "out." {
            i += 1;
            continue;
        }

        // Check for latency pattern: number followed by "ms" or "ms."
        if let Ok(latency) = part.parse::<f64>() {
            if i + 1 < parts.len() {
                let next_part = parts[i + 1].trim_end_matches(|c: char| c == '.' || c == ',');
                if next_part == "ms" {
                    latencies.push(Some(latency));
                    i += 2;
                    continue;
                }
            }
            // Latency without "ms" (Unix format)
            latencies.push(Some(latency));
            i += 1;
            continue;
        }

        // Check for "<1" pattern (less than 1ms)
        if part.starts_with('<') {
            if let Ok(latency) = part[1..].parse::<f64>() {
                latencies.push(Some(latency));
                i += 1;
                if i < parts.len() && parts[i] == "ms" {
                    i += 1;
                }
                continue;
            }
        }

        // Check for "ms" alone - skip it
        if part == "ms" {
            i += 1;
            continue;
        }

        // Check for IP address
        if part.contains('.') || part.contains(':') {
            let potential_ip = part.replace("[", "").replace("]", "");
            if potential_ip.parse::<IpAddr>().is_ok() {
                ip = Some(potential_ip);
            }
        }

        i += 1;
    }

    // Must have at least one latency or one IP to be valid
    if latencies.is_empty() && ip.is_none() {
        return None;
    }

    // Ensure we have exactly 3 latency probes (tracetcp default)
    // If we have fewer, pad with the values we have
    while latencies.len() < 3 {
        if latencies.is_empty() {
            break;
        }
        // Copy the last valid latency
        if let Some(&last) = latencies.last() {
            latencies.push(last);
        }
    }

    let avg_latency = calculate_avg_latency(&latencies);
    let packet_loss = calculate_packet_loss(&latencies);

    log::debug!("Parsed tracetcp hop {}: ip={:?}, latencies={:?}, avg={:?}", hop_number, ip, latencies, avg_latency);

    Some(HopResult {
        hop_number,
        ip,
        hostname: None,
        latencies,
        avg_latency,
        packet_loss,
    })
}

/// Parse a traceroute output line (for ICMP/system command output)
/// Windows tracert format:
///  1     2 ms     3 ms     3 ms  172.20.111.1
///  2     1 ms    <1 ms    1 ms  172.20.255.21
///  3    *        *        *     Request timed out.
fn parse_traceroute_line(line: &str) -> Option<HopResult> {
    // Skip empty lines and header lines
    if line.trim().is_empty()
        || line.contains("Tracing route")
        || line.contains("over a maximum")
        || line.contains("Trace complete")
        || line.contains("traceroute to")
        || line.contains("hops max")
        || line.contains("Tracing TCP")
        || line.contains("on port")
    {
        return None;
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    // First part should be hop number
    let hop_number: u32 = parts[0].parse().ok()?;

    let mut ip: Option<String> = None;
    let mut latencies: Vec<Option<f64>> = Vec::new();

    // Check for "Destination Reached" line (tracetcp format)
    let dest_reached_idx = parts.iter().position(|&p| p == "Destination");
    if let Some(idx) = dest_reached_idx {
        // Parse: "Destination Reached in 9 ms. Connection established to 172.18.13.75"
        // Look for "in X ms" pattern
        if idx + 3 < parts.len() && parts.get(idx + 2) == Some(&"in") {
            for i in (idx + 3)..parts.len() {
                if parts[i] == "ms" && i > idx + 3 {
                    if let Ok(latency) = parts[i - 1].parse::<f64>() {
                        latencies.push(Some(latency));
                    }
                    break;
                }
            }
        }
        // Look for IP at the end
        for part in parts.iter().rev() {
            if part.contains('.') && part.parse::<IpAddr>().is_ok() {
                ip = Some(part.to_string());
                break;
            }
        }

        if !latencies.is_empty() || ip.is_some() {
            let avg_latency = latencies.first().copied().flatten();
            return Some(HopResult {
                hop_number,
                ip,
                hostname: None,
                latencies,
                avg_latency,
                packet_loss: 0.0,
            });
        }
        return None;
    }

    let mut i = 1;
    while i < parts.len() {
        let part = parts[i];

        // Check for timeout
        if part == "*" {
            latencies.push(None);
            i += 1;
            continue;
        }

        // Check for "Request" (part of "Request timed out")
        if part == "Request" || part == "timed" || part == "out." {
            i += 1;
            continue;
        }

        // Check for latency pattern: number followed by "ms"
        if let Ok(latency) = part.parse::<f64>() {
            if i + 1 < parts.len() {
                let next_part = parts[i + 1].trim_end_matches(|c: char| c == '.' || c == ',');
                if next_part == "ms" {
                    latencies.push(Some(latency));
                    i += 2;
                    continue;
                }
            }
            // Latency without "ms" (Unix format)
            latencies.push(Some(latency));
            i += 1;
            continue;
        }

        // Check for "<1" pattern (less than 1ms)
        if part.starts_with('<') {
            if let Ok(latency) = part[1..].parse::<f64>() {
                latencies.push(Some(latency));
                i += 1;
                if i < parts.len() && parts[i] == "ms" {
                    i += 1;
                }
                continue;
            }
        }

        // Check for "ms" alone - skip it
        if part == "ms" {
            i += 1;
            continue;
        }

        // Check for IP address
        if part.contains('.') || part.contains(':') {
            let potential_ip = part.replace("[", "").replace("]", "");
            if potential_ip.parse::<IpAddr>().is_ok() {
                ip = Some(potential_ip);
            }
        }

        i += 1;
    }

    let avg_latency = calculate_avg_latency(&latencies);
    let packet_loss = calculate_packet_loss(&latencies);

    log::debug!("Parsed hop {}: ip={:?}, latencies={:?}, avg={:?}", hop_number, ip, latencies, avg_latency);

    Some(HopResult {
        hop_number,
        ip,
        hostname: None,
        latencies,
        avg_latency,
        packet_loss,
    })
}

/// Emit hop result to frontend
fn emit_hop(app_handle: &tauri::AppHandle, target: &str, hop: &HopResult) {
    let hop_event = HopResultWithTarget {
        target: target.to_string(),
        hop_number: hop.hop_number,
        ip: hop.ip.clone(),
        hostname: hop.hostname.clone(),
        latencies: hop.latencies.clone(),
        avg_latency: hop.avg_latency,
        packet_loss: hop.packet_loss,
    };
    if let Err(e) = app_handle.emit("trace-hop", &hop_event) {
        log::error!("Failed to emit trace hop: {}", e);
    }
}

/// Hop result with target for frontend
#[derive(Debug, Clone, serde::Serialize)]
struct HopResultWithTarget {
    pub target: String,
    pub hop_number: u32,
    pub ip: Option<String>,
    pub hostname: Option<String>,
    pub latencies: Vec<Option<f64>>,
    pub avg_latency: Option<f64>,
    pub packet_loss: f64,
}