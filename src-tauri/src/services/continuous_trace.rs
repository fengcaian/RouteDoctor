// 持续路径监控（mtr / PingPlotter 风格）
//
// 设计（phase 1）：
// - 每 ping_interval_ms 做一次"完整 TTL=1..N 探测"（相当于一次 traceroute）。
// - 每一轮把每一跳的结果（包括超时跳）都作为一条 sample 发出并持久化，
//   由 round_seq 标识轮次。
// - 前端：
//     * "首轮完成"emit continuous-trace-path-discovered（一次性）
//     * "后续每轮"emit continuous-trace-path-update（含最新路径快照，前端 merge）
//     * "每轮每跳"emit continuous-trace-hop-result（含 seq=round_seq）
//     * "新出现的 IP"emit continuous-trace-hop-geo（异步富化 hostname+geo）
//
// 相比旧实现（先 discover 一次→为每个已知 IP 起独立 ping task）的优势：
//   1) 初始超时的跳可以在后续轮次被"补上"（PingPlotter 招牌行为）
//   2) 路径变化（含 ECMP 抖动、运营商切换）天然感知，无需独立 recheck task
//   3) 数据模型只有一个"每轮 traceroute"动作，行为统一

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::{self, Sender};
use tokio::sync::{watch, RwLock};
use tokio::time::MissedTickBehavior;
use crate::error::{AppError, AppResult};
use crate::services::dns::reverse_lookup;
use crate::services::geoip;
use crate::storage::trace_persist::{self, PersistEvent, PersistSample};
use tauri::Emitter;
use tauri::Manager;
use serde::Serialize;

/// 持续路径监控的单跳 Ping 结果（每一轮每一跳一条）
#[derive(Debug, Clone, Serialize)]
pub struct ContinuousTraceHopResult {
    pub target: String,
    pub hop_number: u32,
    /// 该轮该跳响应的 IP；无响应或未探测到时为 None
    pub hop_ip: Option<String>,
    pub latency_ms: Option<f64>,
    pub is_timeout: bool,
    pub timestamp: i64,
    /// 轮次编号（1-based，每次完整 traceroute 递增）
    pub seq: u32,
}

/// 路径发现快照：首轮完成或后续每轮 emit
#[derive(Debug, Clone, Serialize)]
pub struct PathSnapshot {
    pub target: String,
    pub hops: Vec<DiscoveredHop>,
    pub round_seq: u32,
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
static CONTINUOUS_TRACE_SESSIONS: once_cell::sync::Lazy<Arc<RwLock<HashMap<String, ContinuousTraceSession>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

/// TCP 探测的默认目标端口，与 PingPlotter 默认对齐
const DEFAULT_TCP_PORT: u16 = 80;

/// 启动持续路径监控
#[allow(clippy::too_many_arguments)]
pub async fn start_continuous_trace(
    app_handle: tauri::AppHandle,
    target: String,
    max_hops: u32,
    timeout_ms: u32,
    ping_interval_ms: u32,
    probe_method: String,
    persist: bool,
    tcp_port: Option<u16>,
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
        tcp_port.unwrap_or(DEFAULT_TCP_PORT),
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

/// 持续路径监控后台任务（mtr / PingPlotter 风格：每轮完整 TTL 探测）
#[allow(clippy::too_many_arguments)]
async fn continuous_trace_task(
    app_handle: tauri::AppHandle,
    target: String,
    max_hops: u32,
    timeout_ms: u32,
    ping_interval_ms: u32,
    probe_method: String,
    persist: bool,
    tcp_port: u16,
    mut stop_rx: mpsc::Receiver<()>,
) {
    log::info!(
        "Starting continuous trace (mtr-style) to {} (interval: {}ms, method: {}, tcp_port: {}, persist: {})",
        target, ping_interval_ms, probe_method, tcp_port, persist
    );

    // 首轮探测，检测目标是否可达、拿到初始路径快照。
    // 用 select 让首轮 discover 也可被 stop 中断（避免用户在首轮探测阶段
    // 就点了停止但要等 timeout_ms 才生效）。
    let first_round_hops = tokio::select! {
        r = discover_path(&app_handle, &target, max_hops, timeout_ms, &probe_method, tcp_port) => r,
        _ = stop_rx.recv() => {
            log::info!("Continuous trace to {} stopping during first discover", target);
            cleanup_session(&target).await;
            return;
        }
    };

    if first_round_hops.is_empty() {
        log::error!("Failed to discover path to {}", target);
        let _ = app_handle.emit(
            "continuous-trace-error",
            &format!("无法发现到 {} 的路径", target),
        );
        cleanup_session(&target).await;
        return;
    }

    // 持久化：创建 session 行 + 首轮 hop 元信息落盘
    let persist_handle: Option<(i64, mpsc::Sender<PersistEvent>)> = if persist {
        match trace_persist::start_session(
            &app_handle,
            &target,
            ping_interval_ms,
            timeout_ms,
            &probe_method,
        )
        .await
        {
            Ok(session_id) => {
                for (hop_num, ip_opt, hostname, _latency, _is_timeout) in &first_round_hops {
                    let _ = trace_persist::upsert_hop_info(
                        &app_handle,
                        session_id,
                        *hop_num,
                        ip_opt.as_deref(),
                        hostname.as_deref(),
                        None,
                    )
                    .await;
                }
                let _ = app_handle.emit(
                    "continuous-trace-session-started",
                    serde_json::json!({ "target": target, "session_id": session_id }),
                );
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

    // 用于跟踪每跳"曾经出现过的 IP 集合"，识别新出现的 IP → 触发富化 + emit
    let mut known_hop_ips: HashMap<u32, HashSet<String>> = HashMap::new();

    // 首轮完成：emit path-discovered + 富化
    let round_seq_first: u32 = 1;
    emit_round_results(
        &app_handle,
        &target,
        &first_round_hops,
        round_seq_first,
        persist_handle.as_ref(),
    )
    .await;
    let discovered = PathSnapshot {
        target: target.clone(),
        hops: first_round_hops
            .iter()
            .map(|(n, ip, h, _lat, _to)| DiscoveredHop {
                hop_number: *n,
                ip: ip.clone(),
                hostname: h.clone(),
            })
            .collect(),
        round_seq: round_seq_first,
    };
    let _ = app_handle.emit("continuous-trace-path-discovered", &discovered);

    // 收集首轮所有新 IP 并富化
    let mut first_new_ips: Vec<(u32, String)> = Vec::new();
    for (hop_num, ip_opt, _hostname, _latency, _is_timeout) in &first_round_hops {
        if let Some(ip) = ip_opt {
            let entry = known_hop_ips.entry(*hop_num).or_default();
            if entry.insert(ip.clone()) {
                first_new_ips.push((*hop_num, ip.clone()));
            }
        }
    }
    if !first_new_ips.is_empty() {
        enrich_new_ips(
            &app_handle,
            &target,
            &first_new_ips,
            persist_handle.as_ref().map(|(sid, _)| *sid),
        );
    }

    // 主循环：每 ping_interval_ms 跑一轮完整探测。
    // Skip 策略：若一轮探测慢于 interval，直接进下一拍（丢掉错过的 tick），避免堆积。
    let (_cancel_tx, mut cancel_rx) = watch::channel::<bool>(false);
    let mut ticker = tokio::time::interval(Duration::from_millis(ping_interval_ms as u64));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    // 第一个 tick 会立刻返回，需要吃掉（首轮我们已经手动跑了）
    ticker.tick().await;

    let mut round_seq: u32 = round_seq_first;
    loop {
        tokio::select! {
            _ = ticker.tick() => {}
            _ = stop_rx.recv() => {
                log::info!("Continuous trace to {} stopping", target);
                break;
            }
            _ = cancel_rx.changed() => break,
        }

        round_seq = round_seq.wrapping_add(1);
        // 让 discover_path 可被 stop 中断：否则用户点停止后本轮仍会跑完（约 timeout_ms）
        // 并 emit 结果，导致"停止/清除后前端数据延迟复活"。
        let hops = tokio::select! {
            r = discover_path(&app_handle, &target, max_hops, timeout_ms, &probe_method, tcp_port) => r,
            _ = stop_rx.recv() => {
                log::info!("Continuous trace to {} stopping during round {} discover", target, round_seq);
                break;
            }
        };
        if hops.is_empty() {
            log::warn!("[continuous_trace] round {} 未拿到任何跳，跳过本轮", round_seq);
            continue;
        }

        // emit 每跳结果 + 持久化 sample
        emit_round_results(
            &app_handle,
            &target,
            &hops,
            round_seq,
            persist_handle.as_ref(),
        )
        .await;

        // 识别新出现的跳/IP，更新 hop_info + 富化
        let mut new_ips: Vec<(u32, String)> = Vec::new();
        for (hop_num, ip_opt, hostname, _latency, _is_timeout) in &hops {
            if let Some(ip) = ip_opt {
                let entry = known_hop_ips.entry(*hop_num).or_default();
                if entry.insert(ip.clone()) {
                    new_ips.push((*hop_num, ip.clone()));

                    // 落盘/更新 hop_info。当前 schema 每 (session_id,hop_number) 仅
                    // 保留一条 hop_info，多 IP 场景下会被最新观察到的 IP 覆盖——
                    // phase 1 接受这个简化（前端 runtime 内存里能看到多 IP）。
                    if let Some((sid, _)) = persist_handle.as_ref() {
                        let _ = trace_persist::upsert_hop_info(
                            &app_handle,
                            *sid,
                            *hop_num,
                            Some(ip.as_str()),
                            hostname.as_deref(),
                            None,
                        )
                        .await;
                    }
                }
            }
        }

        // 发送本轮完整的路径快照给前端做 merge
        let snapshot = PathSnapshot {
            target: target.clone(),
            hops: hops
                .iter()
                .map(|(n, ip, h, _lat, _to)| DiscoveredHop {
                    hop_number: *n,
                    ip: ip.clone(),
                    hostname: h.clone(),
                })
                .collect(),
            round_seq,
        };
        let _ = app_handle.emit("continuous-trace-path-update", &snapshot);

        if !new_ips.is_empty() {
            enrich_new_ips(
                &app_handle,
                &target,
                &new_ips,
                persist_handle.as_ref().map(|(sid, _)| *sid),
            );
        }
    }

    // 通知 writer：会话结束，flush 剩余样本并标记 status='stopped'
    if let Some((session_id, persist_tx)) = persist_handle {
        let now = chrono::Utc::now().timestamp_millis();
        let _ = persist_tx.send(PersistEvent::SessionStopped(session_id, now)).await;
    }

    cleanup_session(&target).await;
}

/// 一轮探测结果批量 emit + 持久化。
/// hops 元素为 (hop_number, ip_opt, hostname_opt)——但因为 discover_path 只返回
/// IP 和 hostname，我们需要**从 fast_traceroute 结果拿 rtt**。这里的 hops 已经
/// 经过 discover_path 处理，超时跳 ip 为 None，latency 未知（在 emit 时无法反查）。
///
/// 因此我们在 discover_path 层重构了返回类型，把 rtt 一并带出。这里改成接收
/// `Vec<HopResult>` 更合适——见 discover_path 的返回类型。
async fn emit_round_results(
    app_handle: &tauri::AppHandle,
    target: &str,
    hops: &[(u32, Option<String>, Option<String>, Option<f64>, bool)],
    round_seq: u32,
    persist_handle: Option<&(i64, mpsc::Sender<PersistEvent>)>,
) {
    let timestamp = chrono::Utc::now().timestamp_millis();
    for (hop_number, ip_opt, _hostname, latency_ms, is_timeout) in hops {
        let payload = ContinuousTraceHopResult {
            target: target.to_string(),
            hop_number: *hop_number,
            hop_ip: ip_opt.clone(),
            latency_ms: *latency_ms,
            is_timeout: *is_timeout,
            timestamp,
            seq: round_seq,
        };
        let _ = app_handle.emit("continuous-trace-hop-result", &payload);

        // 持久化：sample 带上 IP，支持 ECMP/路径抖动的历史回放（phase 2）
        if let Some((sid, tx)) = persist_handle {
            let sample = PersistSample {
                session_id: *sid,
                hop_number: *hop_number,
                seq: round_seq,
                ip: ip_opt.clone(),
                latency_ms: *latency_ms,
                is_timeout: *is_timeout,
                timestamp,
            };
            if let Err(e) = tx.try_send(PersistEvent::Sample(sample)) {
                log::warn!("trace persist channel full or closed: {:?}", e);
            }
        }
    }
}

/// 异步富化一批新出现的 IP：反向 DNS + GeoIP，然后 emit hop-geo。
/// 每个 IP 一个 tokio task，互不阻塞。
fn enrich_new_ips(
    app_handle: &tauri::AppHandle,
    target: &str,
    new_ips: &[(u32, String)],
    session_id: Option<i64>,
) {
    for (hop_num, ip_str) in new_ips {
        let ah = app_handle.clone();
        let target_owned = target.to_string();
        let hop_num = *hop_num;
        let ip_str = ip_str.clone();

        tokio::spawn(async move {
            let ip_addr: std::net::IpAddr = match ip_str.parse() {
                Ok(a) => a,
                Err(_) => return,
            };
            let (hostname_res, geo_res) =
                tokio::join!(reverse_lookup(&ip_addr), geoip::lookup_one(&ip_addr));
            let hostname = hostname_res.ok().flatten();
            let geo = geo_res;
            let _ = ah.emit(
                "continuous-trace-hop-geo",
                serde_json::json!({
                    "target": target_owned,
                    "hop_number": hop_num,
                    "ip": ip_str,
                    "geo": geo,
                    "hostname": hostname,
                }),
            );

            // 顺便把 hostname/geo 更新到持久化的 hop_info
            if let Some(sid) = session_id {
                let geo_json = geo.as_ref().and_then(|g| serde_json::to_string(g).ok());
                let _ = trace_persist::upsert_hop_info(
                    &ah,
                    sid,
                    hop_num,
                    Some(ip_str.as_str()),
                    hostname.as_deref(),
                    geo_json.as_deref(),
                )
                .await;
            }
        });
    }
}

/// 单轮路径探测：返回每一跳的 (hop_number, ip_opt, hostname_opt, latency_ms, is_timeout)。
///
/// 三级回退（按速度由快到慢）：
/// 1. 对应 probe_method 的并行实现（fast_traceroute / fast_udp / fast_tcp）
/// 2. 并行 ICMP（fast_traceroute）—— UDP/TCP 失败时的兜底
/// 3. 系统命令（tracert/traceroute/tracetcp）—— raw socket 完全不可用时
///
/// 与旧实现的差别：不再在这里富化 hostname/geo（那部分工作交给上层，因为
/// mtr 风格下同一 IP 只需要富化一次，反复 discover 时不应重复查 DNS）。
async fn discover_path(
    app_handle: &tauri::AppHandle,
    target: &str,
    max_hops: u32,
    timeout_ms: u32,
    probe_method: &str,
    tcp_port: u16,
) -> Vec<(u32, Option<String>, Option<String>, Option<f64>, bool)> {
    let t_total = std::time::Instant::now();

    // 解析目标 IPv4（DNS）
    let resolved_ipv4 = resolve_target_ipv4(target).await.ok();

    if let Some(ipv4) = resolved_ipv4 {
        let fast_result: Option<Vec<crate::services::fast_traceroute::FastHop>> = match probe_method {
            "icmp" => match crate::services::fast_traceroute::parallel_icmp_traceroute(
                ipv4, max_hops, timeout_ms,
            )
            .await
            {
                Ok(hops) => Some(hops),
                Err(e) => {
                    log::warn!("Fast ICMP traceroute failed ({}), falling back to system", e);
                    None
                }
            },
            "udp" => {
                #[cfg(windows)]
                {
                    if crate::services::npcap::detect::detect_npcap().installed {
                        match crate::services::npcap::pcap_udp_traceroute::parallel_udp_traceroute(
                            ipv4, max_hops, timeout_ms,
                        )
                        .await
                        {
                            Ok(hops) => Some(hops),
                            Err(e) => {
                                log::warn!("[discover_path:udp] Npcap 失败 ({}), 回退 fast UDP", e);
                                run_fast_udp(ipv4, max_hops, timeout_ms).await
                            }
                        }
                    } else {
                        run_fast_udp(ipv4, max_hops, timeout_ms).await
                    }
                }
                #[cfg(not(windows))]
                {
                    run_fast_udp(ipv4, max_hops, timeout_ms).await
                }
            }
            "tcp" => {
                #[cfg(windows)]
                {
                    if crate::services::npcap::detect::detect_npcap().installed {
                        match crate::services::npcap::pcap_tcp_traceroute::parallel_tcp_traceroute(
                            ipv4, tcp_port, max_hops, timeout_ms,
                        )
                        .await
                        {
                            Ok(hops) => Some(hops),
                            Err(e) => {
                                log::warn!("[discover_path:tcp] Npcap 失败 ({}), 回退 fast TCP", e);
                                run_fast_tcp(ipv4, tcp_port, max_hops, timeout_ms).await
                            }
                        }
                    } else {
                        run_fast_tcp(ipv4, tcp_port, max_hops, timeout_ms).await
                    }
                }
                #[cfg(not(windows))]
                {
                    run_fast_tcp(ipv4, tcp_port, max_hops, timeout_ms).await
                }
            }
            _ => None,
        };

        // 判断 fast 路径是否"实质有效"
        if let Some(fast_hops) = fast_result {
            let target_ip_str = ipv4.to_string();
            let has_intermediate = fast_hops
                .iter()
                .any(|h| h.ip.as_deref().map(|ip| ip != target_ip_str).unwrap_or(false));

            // 只要至少有一个中间跳，就认为 fast 是有效的（否则可能是 Windows raw
            // socket 限制，需要回退）
            if !fast_hops.is_empty() && has_intermediate {
                log::info!(
                    "[discover_path:{}] fast 完成 {} 跳, 总耗时 {:?}",
                    probe_method,
                    fast_hops.len(),
                    t_total.elapsed()
                );
                return fast_hops_to_result(fast_hops);
            }

            log::warn!(
                "[discover_path:{}] fast 仅识别 {} 跳且无中间跳, 尝试 ICMP 兜底",
                probe_method,
                fast_hops.len()
            );
        }

        // UDP/TCP 失败时尝试 ICMP 兜底（Windows 上更可靠）
        if probe_method != "icmp" {
            if let Ok(fast_hops) =
                crate::services::fast_traceroute::parallel_icmp_traceroute(ipv4, max_hops, timeout_ms)
                    .await
            {
                let target_ip_str = ipv4.to_string();
                let has_intermediate = fast_hops
                    .iter()
                    .any(|h| h.ip.as_deref().map(|ip| ip != target_ip_str).unwrap_or(false));
                if !fast_hops.is_empty() && has_intermediate {
                    log::info!(
                        "[discover_path:{}] ICMP 兜底完成 {} 跳",
                        probe_method,
                        fast_hops.len()
                    );
                    return fast_hops_to_result(fast_hops);
                }
            }
        }
    } else {
        log::warn!("Failed to resolve {} to IPv4, using system command", target);
    }

    // 最终回退：系统命令
    log::info!("[discover_path:{}] 回退到系统命令", probe_method);
    let hops =
        discover_path_via_system_command(app_handle, target, max_hops, timeout_ms, probe_method).await;
    log::info!("[discover_path:{}] 系统命令 {:?}", probe_method, t_total.elapsed());
    hops
}

/// FastHop 数组转换为 emit_round_results 期望的元组格式。
/// hostname 这一轮暂时为 None（异步富化会通过 hop-geo 事件补齐）。
fn fast_hops_to_result(
    fast_hops: Vec<crate::services::fast_traceroute::FastHop>,
) -> Vec<(u32, Option<String>, Option<String>, Option<f64>, bool)> {
    fast_hops
        .into_iter()
        .map(|h| {
            let is_timeout = h.ip.is_none();
            (h.hop_number, h.ip, None, h.rtt_ms, is_timeout)
        })
        .collect()
}

/// 拆分 "host:port"，返回 (host, port)。无端口时使用 default_port。
fn split_target_port(target: &str, default_port: u16) -> (String, u16) {
    if let Some(idx) = target.rfind(':') {
        let after = &target[idx + 1..];
        if let Ok(port) = after.parse::<u16>() {
            return (target[..idx].to_string(), port);
        }
    }
    (target.to_string(), default_port)
}

async fn run_fast_udp(
    ipv4: std::net::Ipv4Addr,
    max_hops: u32,
    timeout_ms: u32,
) -> Option<Vec<crate::services::fast_traceroute::FastHop>> {
    match crate::services::fast_udp_traceroute::parallel_udp_traceroute(ipv4, max_hops, timeout_ms)
        .await
    {
        Ok(hops) => Some(hops),
        Err(e) => {
            log::warn!("Fast UDP traceroute failed ({})", e);
            None
        }
    }
}

async fn run_fast_tcp(
    ipv4: std::net::Ipv4Addr,
    port: u16,
    max_hops: u32,
    timeout_ms: u32,
) -> Option<Vec<crate::services::fast_traceroute::FastHop>> {
    match crate::services::fast_tcp_traceroute::parallel_tcp_traceroute(ipv4, port, max_hops, timeout_ms)
        .await
    {
        Ok(hops) => Some(hops),
        Err(e) => {
            log::warn!("Fast TCP traceroute failed ({})", e);
            None
        }
    }
}

/// 把 target 解析为 IPv4 地址。target 可以是 "host"、"host:port"、"ip"、"ip:port"。
async fn resolve_target_ipv4(target: &str) -> AppResult<std::net::Ipv4Addr> {
    use std::net::ToSocketAddrs;

    let (host_only, _port) = split_target_port(target, 0);

    if let Ok(ip) = host_only.parse::<std::net::Ipv4Addr>() {
        return Ok(ip);
    }
    let lookup = format!("{}:0", host_only);
    let target_owned = target.to_string();
    let ipv4 = tokio::task::spawn_blocking(move || -> Option<std::net::Ipv4Addr> {
        lookup
            .to_socket_addrs()
            .ok()?
            .find_map(|addr| match addr.ip() {
                std::net::IpAddr::V4(v4) => Some(v4),
                _ => None,
            })
    })
    .await
    .map_err(|e| AppError::TracerouteError(format!("DNS join 失败: {}", e)))?
    .ok_or_else(|| AppError::TracerouteError(format!("无法解析 {} 到 IPv4", target_owned)))?;
    Ok(ipv4)
}

/// 调用系统 traceroute 命令实现路径发现（回退方案）
async fn discover_path_via_system_command(
    _app_handle: &tauri::AppHandle,
    target: &str,
    max_hops: u32,
    timeout_ms: u32,
    probe_method: &str,
) -> Vec<(u32, Option<String>, Option<String>, Option<f64>, bool)> {
    let mut hops: Vec<(u32, Option<String>, Option<String>, Option<f64>, bool)> = Vec::new();

    let (command, args) = if cfg!(windows) {
        match probe_method {
            "tcp" => (
                "tracetcp",
                vec![
                    format!("{}:443", target),
                    "-m".to_string(),
                    max_hops.to_string(),
                    "-t".to_string(),
                    timeout_ms.to_string(),
                    "-n".to_string(),
                ],
            ),
            _ => (
                "tracert",
                vec![
                    "-d".to_string(),
                    "-h".to_string(),
                    max_hops.to_string(),
                    "-w".to_string(),
                    timeout_ms.to_string(),
                    target.to_string(),
                ],
            ),
        }
    } else {
        let method_flag = match probe_method {
            "udp" => "-U".to_string(),
            "tcp" => "-T".to_string(),
            _ => "-I".to_string(),
        };
        (
            "traceroute",
            vec![
                method_flag,
                "-n".to_string(),
                "-m".to_string(),
                max_hops.to_string(),
                "-w".to_string(),
                format!("{}", timeout_ms / 1000),
                target.to_string(),
            ],
        )
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
                    // 系统命令输出没有延迟信息（这里我们只做兜底），is_timeout 保守设为 false
                    hops.push((hop_num, Some(ip), None, None, false));
                }
            }
        }
        Err(e) => {
            log::error!("Failed to run traceroute for path discovery: {}", e);
        }
    }

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
    let hop_number: u32 = parts[0].parse().ok()?;
    if hop_number == 0 || hop_number > 64 {
        return None;
    }
    for part in parts.iter().rev() {
        let clean = part.replace(['[', ']'], "");
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
