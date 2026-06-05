use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::{watch, RwLock};
use tokio::time::MissedTickBehavior;
use crate::error::{AppError, AppResult};
use crate::services::icmp::ping_once;
use crate::services::dns::reverse_lookup;
use crate::services::geoip;
use crate::storage::trace_persist::{self, PersistEvent, PersistSample};
use tauri::Emitter;
use tauri::Manager;
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
    persist: bool,
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
        persist,
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
    persist: bool,
    mut stop_rx: mpsc::Receiver<()>,
) {
    log::info!("Starting continuous trace to {} (interval: {}ms, method: {}, persist: {})",
        target, ping_interval_ms, probe_method, persist);

    // 第一步：运行 Traceroute 发现路径
    let hops = discover_path(&app_handle, &target, max_hops, timeout_ms, &probe_method).await;

    if hops.is_empty() {
        log::error!("Failed to discover path to {}", target);
        let _ = app_handle.emit("continuous-trace-error", &format!("无法发现到 {} 的路径", target));
        cleanup_session(&target).await;
        return;
    }

    // 持久化层：可选地创建 trace_session 行 + 注册每跳元信息
    let persist_handle: Option<(i64, mpsc::Sender<PersistEvent>)> = if persist {
        match trace_persist::start_session(
            &app_handle, &target, ping_interval_ms, timeout_ms, &probe_method,
        ).await {
            Ok(session_id) => {
                // 写入每跳元信息（IP / hostname；geo 由后续 emit 时补充）
                for (hop_num, ip, hostname) in &hops {
                    let _ = trace_persist::upsert_hop_info(
                        &app_handle, session_id, *hop_num,
                        Some(ip.as_str()),
                        hostname.as_deref(),
                        None,
                    ).await;
                }

                // 通知前端"会话已落盘"，附带 session_id
                let _ = app_handle.emit("continuous-trace-session-started", serde_json::json!({
                    "target": target,
                    "session_id": session_id,
                }));

                // 取出全局 writer Sender（被挂在 app state 上）
                let state = app_handle.state::<crate::TracePersistState>();
                Some((session_id, state.0.clone()))
            }
            Err(e) => {
                log::error!("Failed to create trace_session for {}: {}", target, e);
                None
            }
        }
    } else {
        None
    };

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

    // 第二步：对每一跳独立循环 Ping（每跳一个 task，按自己的节拍）
    //
    // 设计动机：之前的实现是"全跳同步轮询"——所有跳并行 spawn 一次 ping，
    // 等所有结果（包括超时跳的 timeout_ms）返回后才进入 sleep，导致快跳被慢跳拖累。
    // 每跳独立循环后，每个跳点严格按 ping_interval_ms 节拍发出新点，互不干扰。
    let hop_ips: Vec<(u32, String)> = hops.iter()
        .map(|(num, ip, _)| (*num, ip.clone()))
        .collect();

    let interval = Duration::from_millis(ping_interval_ms as u64);

    // 取消通道：调用 send(true) 后，所有持有 Receiver 的 task 通过 changed() 收到信号。
    // 相比 Notify，watch 不要求 task 处于 await notified() 的瞬间状态——
    // 即便错过广播，下一次 changed() 也能立即返回（因为最终值已变）。
    let (cancel_tx, cancel_rx) = watch::channel::<bool>(false);

    // 为每一跳启动一个独立的 ping 循环 task
    let mut hop_handles = Vec::with_capacity(hop_ips.len());
    for (hop_number, hop_ip) in hop_ips {
        let ah = app_handle.clone();
        let target_owned = target.clone();
        let mut cancel_rx = cancel_rx.clone();
        let timeout = timeout_ms;
        let persist_for_hop = persist_handle.clone();

        let handle = tokio::spawn(async move {
            // 每跳独立 ticker：未到 interval 不会发出新点；上一轮 ping 慢于 interval 时丢弃错过的 tick
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            let mut seq: u32 = 0;

            loop {
                // 等下一拍或取消信号
                tokio::select! {
                    _ = ticker.tick() => {}
                    _ = cancel_rx.changed() => return,
                }
                if *cancel_rx.borrow() { return; }

                seq = seq.wrapping_add(1);
                let timestamp = chrono::Utc::now().timestamp_millis();

                // ping 时也要能被取消：避免长 timeout 阻塞退出
                let ping_result = tokio::select! {
                    res = ping_once(&hop_ip, timeout, 64) => Some(res),
                    _ = cancel_rx.changed() => None,
                };

                let res = match ping_result {
                    None => return, // 被取消
                    Some(Ok(r)) => ContinuousTraceHopResult {
                        target: target_owned.clone(),
                        hop_number,
                        hop_ip: hop_ip.clone(),
                        latency_ms: if r.is_timeout { None } else { Some(r.latency_ms.unwrap_or(0.0) as f64) },
                        is_timeout: r.is_timeout,
                        timestamp,
                        seq,
                    },
                    Some(Err(_)) => ContinuousTraceHopResult {
                        target: target_owned.clone(),
                        hop_number,
                        hop_ip: hop_ip.clone(),
                        latency_ms: None,
                        is_timeout: true,
                        timestamp,
                        seq,
                    },
                };

                let _ = ah.emit("continuous-trace-hop-result", &res);

                // 持久化：推到 writer task（fire-and-forget，writer 批量落盘）
                if let Some((session_id, persist_tx)) = &persist_for_hop {
                    let sample = PersistSample {
                        session_id: *session_id,
                        hop_number: res.hop_number,
                        seq: res.seq,
                        latency_ms: res.latency_ms,
                        is_timeout: res.is_timeout,
                        timestamp: res.timestamp,
                    };
                    // try_send：channel 满了就丢弃这条样本（理论上 buffer 2048 + 500ms flush 不会满）
                    if let Err(e) = persist_tx.try_send(PersistEvent::Sample(sample)) {
                        log::warn!("trace persist channel full or closed: {:?}", e);
                    }
                }
            }
        });
        hop_handles.push(handle);
    }

    // 路径重检测 task（独立于 ping 循环，5 分钟一次）
    let recheck_handle = {
        let ah = app_handle.clone();
        let target_owned = target.clone();
        let probe_method_owned = probe_method.clone();
        let mut cancel_rx = cancel_rx.clone();
        let initial_path: Vec<Option<String>> = hops.iter()
            .map(|(_, ip, _)| Some(ip.clone()))
            .collect();

        tokio::spawn(async move {
            let mut last_path = initial_path;
            let recheck_interval = Duration::from_millis(PATH_RECHECK_INTERVAL_MS);
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(recheck_interval) => {}
                    _ = cancel_rx.changed() => return,
                }
                if *cancel_rx.borrow() { return; }

                let new_hops = discover_path(&ah, &target_owned, max_hops, timeout_ms, &probe_method_owned).await;
                if new_hops.is_empty() { continue; }

                let new_path: Vec<Option<String>> = new_hops.iter()
                    .map(|(_, ip, _)| Some(ip.clone()))
                    .collect();

                if last_path != new_path {
                    let _ = ah.emit("path-changed", serde_json::json!({
                        "target": target_owned,
                        "old": last_path.clone(),
                        "new": new_path.clone(),
                        "timestamp": chrono::Utc::now().timestamp_millis(),
                    }));
                    last_path = new_path;
                }
            }
        })
    };

    // 主任务现在只负责等待停止信号，然后广播取消
    let _ = stop_rx.recv().await;
    log::info!("Continuous trace to {} stopping, cancelling all hop tasks", target);
    let _ = cancel_tx.send(true);

    // 等所有子 task 退出
    for handle in hop_handles {
        let _ = handle.await;
    }
    let _ = recheck_handle.await;

    // 通知 writer：会话结束，flush 剩余样本并标记 status='stopped'
    if let Some((session_id, persist_tx)) = persist_handle {
        let now = chrono::Utc::now().timestamp_millis();
        let _ = persist_tx.send(PersistEvent::SessionStopped(session_id, now)).await;
    }

    cleanup_session(&target).await;
}

/// 发现路径：优先使用并行 ICMP raw socket（快），失败时回退到系统 traceroute 命令（兼容）。
/// 同时做反向 DNS + GeoIP。
async fn discover_path(
    app_handle: &tauri::AppHandle,
    target: &str,
    max_hops: u32,
    timeout_ms: u32,
    probe_method: &str,
) -> Vec<(u32, String, Option<String>)> {
    // 第一步：尝试并行 ICMP traceroute（仅 ICMP 模式可用）
    // UDP/TCP 模式直接走系统命令，因为我们没实现 UDP/TCP 的并行版本
    if probe_method == "icmp" {
        // 解析目标到 IPv4
        if let Ok(ipv4) = resolve_target_ipv4(target).await {
            log::info!("Trying parallel ICMP traceroute to {} ({})", target, ipv4);
            match crate::services::fast_traceroute::parallel_icmp_traceroute(ipv4, max_hops, timeout_ms).await {
                Ok(fast_hops) => {
                    // 组装与原格式一致的结构（hop_number, ip, hostname=None）
                    let mut hops: Vec<(u32, String, Option<String>)> = fast_hops
                        .into_iter()
                        .filter_map(|h| h.ip.map(|ip| (h.hop_number, ip, None)))
                        .collect();

                    if !hops.is_empty() {
                        log::info!("Fast traceroute resolved {} hops with IPs", hops.len());
                        // 反向 DNS + GeoIP
                        enrich_hops_async(app_handle, target, &mut hops).await;
                        return hops;
                    } else {
                        log::warn!("Fast traceroute returned no hops, falling back to system command");
                    }
                }
                Err(e) => {
                    log::warn!("Fast traceroute failed ({}), falling back to system command", e);
                }
            }
        } else {
            log::warn!("Failed to resolve {} to IPv4, using system command", target);
        }
    }

    // 回退到原有的"调系统 tracert/traceroute 命令"实现
    discover_path_via_system_command(app_handle, target, max_hops, timeout_ms, probe_method).await
}

/// 把目标解析为 IPv4 地址
async fn resolve_target_ipv4(target: &str) -> AppResult<std::net::Ipv4Addr> {
    use std::net::ToSocketAddrs;
    // 先试纯 IP 格式
    if let Ok(ip) = target.parse::<std::net::Ipv4Addr>() {
        return Ok(ip);
    }
    // 域名解析
    let host = format!("{}:0", target);
    let target_owned = target.to_string();
    let ipv4 = tokio::task::spawn_blocking(move || -> Option<std::net::Ipv4Addr> {
        host.to_socket_addrs().ok()?
            .find_map(|addr| match addr.ip() {
                std::net::IpAddr::V4(v4) => Some(v4),
                _ => None,
            })
    }).await
        .map_err(|e| AppError::TracerouteError(format!("DNS join 失败: {}", e)))?
        .ok_or_else(|| AppError::TracerouteError(format!("无法解析 {} 到 IPv4", target_owned)))?;
    Ok(ipv4)
}

/// 给已发现的跳异步补充反向 DNS + GeoIP 信息（直接修改 hostname 字段，并 emit hop-geo 事件）
async fn enrich_hops_async(
    app_handle: &tauri::AppHandle,
    target: &str,
    hops: &mut Vec<(u32, String, Option<String>)>,
) {
    let mut handles = Vec::new();
    for (hop_num, ip, _) in hops.iter() {
        let ip_str = ip.clone();
        let hop_num = *hop_num;
        let target_owned = target.to_string();
        let ah = app_handle.clone();

        handles.push(tokio::spawn(async move {
            let ip_addr: std::net::IpAddr = match ip_str.parse() {
                Ok(a) => a,
                Err(_) => return (hop_num, None),
            };
            let (hostname_res, geo_res) = tokio::join!(
                reverse_lookup(&ip_addr),
                geoip::lookup_one(&ip_addr)
            );
            let hostname = hostname_res.ok().flatten();
            if let Some(geo) = geo_res {
                let _ = ah.emit("continuous-trace-hop-geo", serde_json::json!({
                    "target": target_owned,
                    "hop_number": hop_num,
                    "ip": ip_str,
                    "geo": geo,
                    "hostname": hostname,
                }));
            } else if hostname.is_some() {
                let _ = ah.emit("continuous-trace-hop-geo", serde_json::json!({
                    "target": target_owned,
                    "hop_number": hop_num,
                    "ip": ip_str,
                    "geo": null,
                    "hostname": hostname,
                }));
            }
            (hop_num, hostname)
        }));
    }

    for handle in handles {
        if let Ok((hop_num, hostname)) = handle.await {
            if let Some(slot) = hops.iter_mut().find(|(n, _, _)| *n == hop_num) {
                slot.2 = hostname;
            }
        }
    }
}

/// 调用系统 traceroute 命令实现路径发现（回退方案）
async fn discover_path_via_system_command(
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

    // 反向 DNS + GeoIP（共享 enrich 逻辑）
    enrich_hops_async(app_handle, target, &mut hops).await;

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
