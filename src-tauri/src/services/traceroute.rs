use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::RwLock;
use crate::models::trace::{TracerouteResult, HopResult, TracerouteConfig, ProbeMethod};
use crate::services::dns::{resolve, reverse_lookup};
use crate::error::{AppError, AppResult};
use tauri::Emitter;
use socket2::{Socket, Protocol, Type, Domain};
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
    let mut result = TracerouteResult {
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

    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let output = cmd.output().await;

    match output {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                log::error!("Traceroute stderr: {}", stderr);
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let lines: Vec<&str> = stdout.lines().collect();

            for line in lines {
                if stop_rx.try_recv() != Err(mpsc::error::TryRecvError::Empty) {
                    break;
                }

                if let Some(hop) = parse_traceroute_line(line) {
                    process_hop(&app_handle, &config.target, hop, &mut result).await;
                }
            }

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

/// UDP traceroute - implemented in Rust
async fn run_udp_traceroute(
    app_handle: tauri::AppHandle,
    config: TracerouteConfig,
    target_ip: IpAddr,
    mut stop_rx: Receiver<()>,
) {
    let start_time = chrono::Utc::now().timestamp_millis();
    let probe_method = ProbeMethod::Udp;

    // Create session in database
    let session_id = crate::storage::database::create_ping_session(
        &app_handle,
        &config.target,
        "traceroute",
    ).await.unwrap_or(0);

    log::info!("UDP traceroute to {} (max hops: {})", config.target, config.max_hops);

    let mut result = TracerouteResult {
        target: config.target.clone(),
        target_ip: target_ip.to_string(),
        hops: Vec::new(),
        completed: false,
        start_time,
        end_time: None,
        probe_method,
    };

    // UDP traceroute implementation
    let timeout = Duration::from_millis(config.timeout_ms as u64);
    let probes_per_hop = config.probes_per_hop;

    for ttl in 1..=config.max_hops {
        // Check for stop signal
        if stop_rx.try_recv() != Err(mpsc::error::TryRecvError::Empty) {
            log::info!("UDP traceroute stopped at hop {}", ttl);
            break;
        }

        let hop_result = probe_hop_udp(&target_ip, ttl, timeout, probes_per_hop).await;

        // Check if we reached the target
        let reached_target = hop_result.ip.as_ref()
            .map(|ip| ip == &target_ip.to_string())
            .unwrap_or(false);

        process_hop(&app_handle, &config.target, hop_result.clone(), &mut result).await;

        if reached_target {
            log::info!("UDP traceroute reached target at hop {}", ttl);
            result.completed = true;
            break;
        }
    }

    if result.hops.len() == config.max_hops as usize {
        result.completed = true;
    }

    finalize_result(&app_handle, result, &config.target, session_id).await;
}

/// TCP traceroute - implemented in Rust
async fn run_tcp_traceroute(
    app_handle: tauri::AppHandle,
    config: TracerouteConfig,
    target_ip: IpAddr,
    mut stop_rx: Receiver<()>,
) {
    let start_time = chrono::Utc::now().timestamp_millis();
    let probe_method = ProbeMethod::Tcp;

    // Create session in database
    let session_id = crate::storage::database::create_ping_session(
        &app_handle,
        &config.target,
        "traceroute",
    ).await.unwrap_or(0);

    log::info!("TCP traceroute to {} (max hops: {})", config.target, config.max_hops);

    let mut result = TracerouteResult {
        target: config.target.clone(),
        target_ip: target_ip.to_string(),
        hops: Vec::new(),
        completed: false,
        start_time,
        end_time: None,
        probe_method,
    };

    // TCP traceroute - uses TCP SYN packets
    // Default port for TCP traceroute
    let target_port: u16 = 80;
    let timeout = Duration::from_millis(config.timeout_ms as u64);
    let probes_per_hop = config.probes_per_hop;

    for ttl in 1..=config.max_hops {
        // Check for stop signal
        if stop_rx.try_recv() != Err(mpsc::error::TryRecvError::Empty) {
            log::info!("TCP traceroute stopped at hop {}", ttl);
            break;
        }

        let hop_result = probe_hop_tcp(&target_ip, target_port, ttl, timeout, probes_per_hop).await;

        // Check if we reached the target (got SYN-ACK or connection established)
        let reached_target = hop_result.ip.as_ref()
            .map(|ip| ip == &target_ip.to_string())
            .unwrap_or(false);

        process_hop(&app_handle, &config.target, hop_result.clone(), &mut result).await;

        if reached_target {
            log::info!("TCP traceroute reached target at hop {}", ttl);
            result.completed = true;
            break;
        }
    }

    if result.hops.len() == config.max_hops as usize {
        result.completed = true;
    }

    finalize_result(&app_handle, result, &config.target, session_id).await;
}

/// Probe a hop using UDP
async fn probe_hop_udp(target_ip: &IpAddr, ttl: u32, timeout: Duration, probes: u32) -> HopResult {
    let mut latencies: Vec<Option<f64>> = Vec::new();
    let mut last_ip: Option<String> = None;

    let domain = match target_ip {
        IpAddr::V4(_) => Domain::IPV4,
        IpAddr::V6(_) => Domain::IPV6,
    };

    // Use a high port that's likely to be unused
    let target_port: u16 = 33434 + ttl as u16; // Standard traceroute port range

    for _ in 0..probes {
        let probe_result = send_udp_probe(target_ip, target_port, ttl, domain, timeout).await;

        match probe_result {
            Ok((ip, latency_ms)) => {
                latencies.push(Some(latency_ms));
                if ip.is_some() {
                    last_ip = ip;
                }
            }
            Err(_) => {
                latencies.push(None);
            }
        }
    }

    let avg_latency = calculate_avg_latency(&latencies);
    let packet_loss = calculate_packet_loss(&latencies);

    HopResult {
        hop_number: ttl,
        ip: last_ip,
        hostname: None,
        latencies,
        avg_latency,
        packet_loss,
    }
}

/// Probe a hop using TCP SYN
async fn probe_hop_tcp(target_ip: &IpAddr, port: u16, ttl: u32, timeout: Duration, probes: u32) -> HopResult {
    let mut latencies: Vec<Option<f64>> = Vec::new();
    let mut last_ip: Option<String> = None;

    for _ in 0..probes {
        let probe_result = send_tcp_probe(target_ip, port, ttl, timeout).await;

        match probe_result {
            Ok((ip, latency_ms)) => {
                latencies.push(Some(latency_ms));
                if ip.is_some() {
                    last_ip = ip;
                }
            }
            Err(_) => {
                latencies.push(None);
            }
        }
    }

    let avg_latency = calculate_avg_latency(&latencies);
    let packet_loss = calculate_packet_loss(&latencies);

    HopResult {
        hop_number: ttl,
        ip: last_ip,
        hostname: None,
        latencies,
        avg_latency,
        packet_loss,
    }
}

/// Send UDP probe and wait for response
async fn send_udp_probe(
    target_ip: &IpAddr,
    port: u16,
    ttl: u32,
    domain: Domain,
    timeout: Duration,
) -> Result<(Option<String>, f64), Box<dyn std::error::Error + Send + Sync>> {
    use tokio::time::Instant;

    let socket = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;
    socket.set_ttl(ttl)?;
    socket.set_nonblocking(true)?;

    let target_addr = SocketAddr::new(*target_ip, port);

    // Convert to tokio socket
    let std_socket = std::net::UdpSocket::from(socket);
    let udp_socket = tokio::net::UdpSocket::from_std(std_socket)?;

    let start = Instant::now();

    // Send empty UDP packet
    udp_socket.send_to(&[0], target_addr).await?;

    // Wait for response (ICMP Time Exceeded or Port Unreachable)
    let mut buf = [0u8; 512];

    let result = tokio::time::timeout(timeout, udp_socket.recv_from(&mut buf)).await;

    match result {
        Ok(Ok((_, src_addr))) => {
            let latency_ms = start.elapsed().as_millis() as f64;
            Ok((Some(src_addr.ip().to_string()), latency_ms))
        }
        Ok(Err(e)) => {
            log::debug!("UDP probe recv error: {}", e);
            Err(e.into())
        }
        Err(_) => {
            // Timeout - no response
            Err("timeout".into())
        }
    }
}

/// Send TCP SYN probe and wait for response
async fn send_tcp_probe(
    target_ip: &IpAddr,
    port: u16,
    ttl: u32,
    _timeout: Duration,
) -> Result<(Option<String>, f64), Box<dyn std::error::Error + Send + Sync>> {
    // Create TCP socket with TTL set
    let domain = match target_ip {
        IpAddr::V4(_) => Domain::IPV4,
        IpAddr::V6(_) => Domain::IPV6,
    };

    let target_addr = SocketAddr::new(*target_ip, port);
    let target_ip_str = target_ip.to_string();

    // Run blocking socket operations in a blocking task
    let result: Result<Result<(Option<String>, f64), Box<dyn std::error::Error + Send + Sync>>, tokio::task::JoinError> =
        tokio::task::spawn_blocking(move || {
            let socket = Socket::new(domain, Type::STREAM, Some(Protocol::TCP))?;
            socket.set_ttl(ttl)?;
            socket.set_nonblocking(true)?;

            let start = std::time::Instant::now();

            // Attempt to connect
            match socket.connect(&target_addr.into()) {
                Ok(_) => {
                    let latency_ms = start.elapsed().as_millis() as f64;
                    // Get peer address
                    let peer_addr: Option<String> = socket.peer_addr()
                        .ok()
                        .and_then(|a| a.as_socket().map(|s| s.ip().to_string()));
                    Ok((peer_addr, latency_ms))
                }
                Err(e) => {
                    // On Windows, connection errors might indicate ICMP Time Exceeded
                    // Error codes like WSAETIMEDOUT (10060) = timeout
                    // WSAECONNREFUSED (10061) = reached target but port closed
                    let latency_ms = start.elapsed().as_millis() as f64;

                    let kind = e.kind();
                    match kind {
                        std::io::ErrorKind::TimedOut => {
                            // TTL exceeded - no IP info available without raw socket
                            Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        }
                        std::io::ErrorKind::ConnectionRefused => {
                            // Reached target! Port is closed but we got there
                            Ok((Some(target_ip_str), latency_ms))
                        }
                        _ => {
                            // Other error - might be TTL exceeded
                            Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>)
                        }
                    }
                }
            }
        }).await;

    match result {
        Ok(Ok(r)) => Ok(r),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(Box::new(e) as Box<dyn std::error::Error + Send + Sync>),
    }
}

/// Process a hop result and emit to frontend
async fn process_hop(
    app_handle: &tauri::AppHandle,
    target: &str,
    hop: HopResult,
    result: &mut TracerouteResult,
) {
    // Try to get hostname via reverse DNS
    if hop.ip.is_some() {
        let ip_str = hop.ip.clone().unwrap();
        if let Ok(ip) = ip_str.parse::<IpAddr>() {
            if let Ok(Some(hostname)) = reverse_lookup(&ip).await {
                let hop_with_hostname = HopResult {
                    hostname: Some(hostname),
                    ..hop.clone()
                };
                result.hops.push(hop_with_hostname.clone());
                emit_hop(app_handle, target, &hop_with_hostname);
                return;
            }
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

/// Parse a traceroute output line (for ICMP/system command output)
fn parse_traceroute_line(line: &str) -> Option<HopResult> {
    // Windows tracert format:
    //  1     2 ms     3 ms     3 ms  172.20.111.1
    //  2     1 ms    <1 ms    1 ms  172.20.255.21
    //  3    *        *        *     Request timed out.

    // Unix traceroute format:
    //  1  192.168.1.1  1.234 ms  1.123 ms  1.456 ms

    // Skip empty lines and header lines
    if line.trim().is_empty()
        || line.contains("Tracing route")
        || line.contains("over a maximum")
        || line.contains("Trace complete")
        || line.contains("traceroute to")
        || line.contains("hops max")
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
            if i + 1 < parts.len() && parts[i + 1] == "ms" {
                latencies.push(Some(latency));
                i += 2;
                continue;
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