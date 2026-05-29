use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::RwLock;
use crate::error::{AppError, AppResult};
use crate::services::icmp::ping_once;
use crate::services::dns::reverse_lookup;
use crate::services::geoip;
use tauri::Emitter;
use serde::Serialize;

/// 持续路径监控的单跳 Ping 结果
#[derive(Debug, Clone, Serialize)]
pub struct ContinuousTraceHopResult {
    pub target: String,
    pub hop_number: u32,
    pub hop_ip: String,
    pub latency_ms: Option<f64>,
    pub is_timeout: bool,
    pub timestamp: i64,
    pub seq: u32,
}

/// 路径发现完成事件
#[derive(Debug, Clone, Serialize)]
pub struct PathDiscovered {
    pub target: String,
    pub hops: Vec<DiscoveredHop>,
}

/// 发现的跳信息
#[derive(Debug, Clone, Serialize)]
pub struct DiscoveredHop {
    pub hop_number: u32,
    pub ip: Option<String>,
    pub hostname: Option<String>,
}

/// 会话信息
struct ContinuousTraceSession {
    stop_tx: Sender<()>,
}

/// 活跃的持续路径监控会话
static CONTINUOUS_TRACE_SESSIONS: once_cell::sync::Lazy<Arc<RwLock<std::collections::HashMap<String, ContinuousTraceSession>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(std::collections::HashMap::new())));

/// Default interval between path re-checks (5 minutes)
const PATH_RECHECK_INTERVAL_MS: u64 = 300_000;

/// 启动持续路径监控
pub async fn start_continuous_trace(
    app_handle: tauri::AppHandle,
    target: String,
    max_hops: u32,
    timeout_ms: u32,
    ping_interval_ms: u32,
    probe_method: String,
) -> AppResult<()> {
    // 检查是否已在运行
    let sessions = CONTINUOUS_TRACE_SESSIONS.read().await;
    if sessions.contains_key(&target) {
        return Err(AppError::TracerouteError("持续路径监控已在运行".into()));
    }
    drop(sessions);

    // 创建停止通道
    let (stop_tx, stop_rx) = mpsc::channel::<()>(1);

    // 注册会话
    let mut sessions = CONTINUOUS_TRACE_SESSIONS.write().await;
    sessions.insert(target.clone(), ContinuousTraceSession { stop_tx });
    drop(sessions);

    // 启动后台任务
    tokio::spawn(continuous_trace_task(
        app_handle,
        target,
        max_hops,
        timeout_ms,
        ping_interval_ms,
        probe_method,
        stop_rx,
    ));

    Ok(())
}

/// 停止持续路径监控
pub async fn stop_continuous_trace(app_handle: &tauri::AppHandle, target: &str) -> AppResult<()> {
    let mut sessions = CONTINUOUS_TRACE_SESSIONS.write().await;
    if let Some(session) = sessions.remove(target) {
        let _ = session.stop_tx.send(()).await;
        // 通知前端已停止
        let _ = app_handle.emit("continuous-trace-stopped", target);
    }
    Ok(())
}

/// 持续路径监控后台任务
async fn continuous_trace_task(
    app_handle: tauri::AppHandle,
    target: String,
    max_hops: u32,
    timeout_ms: u32,
    ping_interval_ms: u32,
    probe_method: String,
    mut stop_rx: mpsc::Receiver<()>,
) {
    log::info!("Starting continuous trace to {} (interval: {}ms, method: {})", target, ping_interval_ms, probe_method);

    // 第一步：运行 Traceroute 发现路径
    let hops = discover_path(&app_handle, &target, max_hops, timeout_ms, &probe_method).await;

    if hops.is_empty() {
        log::error!("Failed to discover path to {}", target);
        let _ = app_handle.emit("continuous-trace-error", &format!("无法发现到 {} 的路径", target));
        cleanup_session(&target).await;
        return;
    }

    // 通知前端路径已发现
    let discovered = PathDiscovered {
        target: target.clone(),
        hops: hops.iter().map(|h| DiscoveredHop {
            hop_number: h.0,
            ip: Some(h.1.clone()),
            hostname: h.2.clone(),
        }).collect(),
    };
    let _ = app_handle.emit("continuous-trace-path-discovered", &discovered);

    // 第二步：对每一跳持续 Ping
    let hop_ips: Vec<(u32, String)> = hops.iter()
        .map(|(num, ip, _)| (*num, ip.clone()))
        .collect();

    let interval = Duration::from_millis(ping_interval_ms as u64);
    let timeout = timeout_ms;
    let mut seq: u32 = 0;

    // Path re-check state
    let last_path: Vec<Option<String>> = hop_ips.iter()
        .map(|(_, ip)| Some(ip.clone()))
        .collect();
    let mut last_recheck = std::time::Instant::now();
    loop {
        // 检查停止信号
        if stop_rx.try_recv().is_ok() {
            log::info!("Continuous trace to {} stopped by user", target);
            break;
        }

        seq += 1;
        let timestamp = chrono::Utc::now().timestamp_millis();

        // 对每一跳并行 Ping
        let mut handles = Vec::new();
        for (hop_number, hop_ip) in &hop_ips {
            let hop_ip = hop_ip.clone();
            let hop_number = *hop_number;
            let timeout = timeout;

            handles.push(tokio::spawn(async move {
                let result = ping_once(&hop_ip, timeout, 64).await;
                (hop_number, hop_ip, result)
            }));
        }

        // 收集结果并发送事件
        for handle in handles {
            if let Ok((hop_number, hop_ip, result)) = handle.await {
                let hop_result = match result {
                    Ok(ping_result) => ContinuousTraceHopResult {
                        target: target.clone(),
                        hop_number,
                        hop_ip,
                        latency_ms: if ping_result.is_timeout { None } else { Some(ping_result.latency_ms.unwrap_or(0.0) as f64) },
                        is_timeout: ping_result.is_timeout,
                        timestamp,
                        seq,
                    },
                    Err(_) => ContinuousTraceHopResult {
                        target: target.clone(),
                        hop_number,
                        hop_ip,
                        latency_ms: None,
                        is_timeout: true,
                        timestamp,
                        seq,
                    },
                };

                let _ = app_handle.emit("continuous-trace-hop-result", &hop_result);
            }
        }

        // Periodic path re-check (non-blocking)
        if last_recheck.elapsed().as_millis() >= PATH_RECHECK_INTERVAL_MS as u128 {
            last_recheck = std::time::Instant::now();
            let ah = app_handle.clone();
            let tgt = target.clone();
            let mh = max_hops;
            let tm = timeout_ms;
            let pm = probe_method.clone();
            let old_path = last_path.clone();

            tokio::spawn(async move {
                let new_hops = discover_path(&ah, &tgt, mh, tm, &pm).await;
                if !new_hops.is_empty() {
                    let new_path: Vec<Option<String>> = new_hops.iter()
                        .map(|(_, ip, _)| Some(ip.clone()))
                        .collect();

                    // Diff old vs new path
                    if old_path != new_path {
                        let old_ips: Vec<Option<String>> = old_path.clone();
                        let new_ips = new_path.clone();
                        let _ = ah.emit("path-changed", serde_json::json!({
                            "target": tgt,
                            "old": old_ips,
                            "new": new_ips,
                            "timestamp": chrono::Utc::now().timestamp_millis(),
                        }));
                    }
                }
            });
        }

        // 等待下一个间隔
        tokio::select! {
            _ = tokio::time::sleep(interval) => {},
            _ = stop_rx.recv() => {
                log::info!("Continuous trace to {} stopped during sleep", target);
                break;
            }
        }
    }

    cleanup_session(&target).await;
}

/// 发现路径：运行 traceroute 并提取每一跳的 IP，同时做反向 DNS + GeoIP
async fn discover_path(
    app_handle: &tauri::AppHandle,
    target: &str,
    max_hops: u32,
    timeout_ms: u32,
    probe_method: &str,
) -> Vec<(u32, String, Option<String>)> {
    let mut hops: Vec<(u32, String, Option<String>)> = Vec::new();

    // 根据探测方式选择命令参数
    let (command, args) = if cfg!(windows) {
        match probe_method {
            "tcp" => {
                ("tracetcp", vec![
                    format!("{}:443", target),
                    "-m".to_string(), max_hops.to_string(),
                    "-t".to_string(), timeout_ms.to_string(),
                    "-n".to_string(),
                ])
            }
            _ => {
                ("tracert", vec![
                    "-d".to_string(),
                    "-h".to_string(), max_hops.to_string(),
                    "-w".to_string(), timeout_ms.to_string(),
                    target.to_string(),
                ])
            }
        }
    } else {
        let method_flag = match probe_method {
            "udp" => "-U".to_string(),
            "tcp" => "-T".to_string(),
            _ => "-I".to_string(),
        };
        ("traceroute", vec![
            method_flag,
            "-n".to_string(),
            "-m".to_string(), max_hops.to_string(),
            "-w".to_string(), format!("{}", timeout_ms / 1000),
            target.to_string(),
        ])
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

    match cmd.output().await {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if let Some((hop_num, ip)) = parse_hop_ip(line) {
                    hops.push((hop_num, ip, None));
                }
            }
        }
        Err(e) => {
            log::error!("Failed to run traceroute for path discovery: {}", e);
        }
    }

    // Do reverse DNS + GeoIP for each hop in parallel
    let mut geo_handles = Vec::new();
    for (hop_num, ip, _) in &hops {
        let ip_str = ip.clone();
        let hop_num = *hop_num;
        let target_owned = target.to_string();
        let ah = app_handle.clone();

        geo_handles.push(tokio::spawn(async move {
            let ip_addr: std::net::IpAddr = ip_str.parse().ok()?;
            let (hostname_res, geo_res) = tokio::join!(
                reverse_lookup(&ip_addr),
                geoip::lookup_one(&ip_addr)
            );

            let hostname = hostname_res.ok().flatten();

            // Emit geo info to frontend
            if let Some(geo) = geo_res {
                let _ = ah.emit("continuous-trace-hop-geo", serde_json::json!({
                    "target": target_owned,
                    "hop_number": hop_num,
                    "ip": ip_str,
                    "geo": geo,
                    "hostname": hostname,
                }));
            } else if hostname.is_some() {
                // Still emit just the hostname even without geo
                let _ = ah.emit("continuous-trace-hop-geo", serde_json::json!({
                    "target": target_owned,
                    "hop_number": hop_num,
                    "ip": ip_str,
                    "geo": null,
                    "hostname": hostname,
                }));
            }

            Some(hostname)
        }));
    }

    // Wait for all geo lookups and update hostnames
    for (i, handle) in geo_handles.into_iter().enumerate() {
        if let Ok(Some(hostname)) = handle.await {
            hops[i].2 = hostname;
        }
    }

    // 通知前端发现进度
    let _ = app_handle.emit("continuous-trace-discovering", &format!("发现 {} 跳", hops.len()));

    hops
}

/// 从 tracert 输出行中提取跳数和 IP
fn parse_hop_ip(line: &str) -> Option<(u32, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }

    // 第一个部分应该是跳数
    let hop_number: u32 = parts[0].parse().ok()?;
    if hop_number == 0 || hop_number > 64 {
        return None;
    }

    // 查找 IP 地址（通常是最后一个看起来像 IP 的部分）
    for part in parts.iter().rev() {
        let clean = part.replace("[", "").replace("]", "");
        if clean.parse::<std::net::IpAddr>().is_ok() {
            return Some((hop_number, clean));
        }
    }

    None
}

/// 清理会话
async fn cleanup_session(target: &str) {
    let mut sessions = CONTINUOUS_TRACE_SESSIONS.write().await;
    sessions.remove(target);
}
