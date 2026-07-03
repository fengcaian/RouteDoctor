// 并行 UDP traceroute（PingPlotter / mtr 风格 + 独立 recv 线程）
//
// 原理：
// - 为 TTL=1..max_hops 各开一个 UDP socket，设置 IP_TTL=ttl，向目标的端口
//   33434+ttl 发送一个最小数据包（标准 traceroute 使用的端口范围）。
// - 中间路由器返回 ICMP Time Exceeded(type=11)；目标主机不监听这些端口，
//   返回 ICMP Destination Unreachable(type=3, code=3, Port Unreachable)。
// - 用一个 raw ICMP socket 统一接收所有 ICMP 回复，通过解析 ICMP 包内嵌的
//   "原始 IP+UDP 头"提取 dst_port，反查 ttl = dst_port - 33434。
//
// 关键：send 和 recv 分离
// - 独立线程一开始就在 recv_from 上等 ICMP，收到立即用 Instant::now() 打
//   时间戳并通过 mpsc 发给主线程 → 首跳 RTT 不再被 send 阶段的耗时污染。
// - Windows 上"每 TTL 一个新 UdpSocket bind"耗时约 1200ms，如果同步阻塞
//   直到发完再收，首跳 RTT 会虚高 ~1200ms（bug 现场）。
// - 主线程负责发 30 个 UDP 包 + 从 channel 收响应事件 + 更新状态。
// - 保留了每 TTL 独立 src_port 的语义（能看到 UDP ECMP 分岔的多 IP 现象）。

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use socket2::{Domain, Protocol, Socket, Type};
use crate::error::{AppError, AppResult};
use crate::services::fast_traceroute::FastHop;

/// 标准 traceroute 起始端口，与 Linux/BSD 的 traceroute 默认一致。
/// 我们用 BASE+ttl 来反查响应对应的 TTL。
const BASE_PORT: u16 = 33434;

/// 一条 ICMP 响应事件：由 recv 线程实时打时间戳，通过 mpsc 发给主线程处理
struct RecvEvent {
    recv_time: Instant,
    src_ip: Ipv4Addr,
    ttl: u8,
    #[allow(dead_code)]
    icmp_type: u8,
}

/// 并行 UDP traceroute。target 必须是已解析的 IPv4 地址。
/// 整体耗时 = max(网络最大 RTT, timeout_ms)
pub async fn parallel_udp_traceroute(
    target: Ipv4Addr,
    max_hops: u32,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let max_hops = max_hops.clamp(1, 64) as u8;

    // 接收 ICMP 错误回复需要 raw ICMP socket（与 fast_traceroute 一致）。
    let icmp_socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))
        .map_err(|e| AppError::TracerouteError(format!(
            "无法创建 raw ICMP socket（UDP traceroute 需要管理员权限）: {}", e
        )))?;

    // 显式 bind 到 0.0.0.0:0：Windows 上 raw socket 在 recv_from 前必须先 bind
    // （否则会立刻返回 WSAEINVAL=10022，让 read_timeout 失效从而 spin）。
    let bind_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into();
    if let Err(e) = icmp_socket.bind(&bind_addr.into()) {
        return Err(AppError::TracerouteError(format!(
            "raw ICMP socket bind 失败: {}", e
        )));
    }

    icmp_socket.set_nonblocking(false).ok();
    icmp_socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .ok();

    let result = tokio::task::spawn_blocking(move || -> AppResult<Vec<FastHop>> {
        do_udp_traceroute_blocking(icmp_socket, target, max_hops, timeout_ms)
    })
    .await
    .map_err(|e| AppError::TracerouteError(format!("UDP traceroute task join 失败: {}", e)))??;

    Ok(result)
}

fn do_udp_traceroute_blocking(
    icmp_socket: Socket,
    target: Ipv4Addr,
    max_hops: u8,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_recv = stop_flag.clone();
    let (tx, rx) = mpsc::channel::<RecvEvent>();

    // ===== 独立 recv 线程 =====
    // 一开始就在 icmp_socket.recv_from 上等 ICMP。收到即用 Instant::now() 打时间戳,
    // 走过滤 + 反查 TTL 后通过 mpsc 发给主线程。
    // 目的：send 阶段的耗时（Windows 上每 TTL 独立 UdpSocket bind 约 1000ms）
    //      不再污染 RTT 计算——recv_time 是包真正到达的时刻。
    let recv_thread = std::thread::spawn(move || {
        let mut buf = [std::mem::MaybeUninit::new(0u8); 1500];
        while !stop_flag_recv.load(Ordering::Relaxed) {
            match icmp_socket.recv_from(&mut buf) {
                Ok((len, src)) => {
                    let recv_time = Instant::now();
                    let data = unsafe {
                        std::slice::from_raw_parts(buf.as_ptr() as *const u8, len)
                    };
                    let src_addr: SocketAddr = match src.as_socket() {
                        Some(a) => a,
                        None => continue,
                    };
                    let src_ip = match src_addr.ip() {
                        IpAddr::V4(v4) => v4,
                        _ => continue,
                    };
                    if let Some((icmp_type, ttl)) = parse_icmp_for_udp(data, target) {
                        // 主线程 rx 已关闭 → 我们也退出
                        if tx.send(RecvEvent {
                            recv_time,
                            src_ip,
                            ttl,
                            icmp_type,
                        }).is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    // WouldBlock / TimedOut = read_timeout=50ms 到期,正常
                    if e.kind() != std::io::ErrorKind::WouldBlock
                        && e.kind() != std::io::ErrorKind::TimedOut
                    {
                        log::warn!("UDP traceroute recv thread error: {}", e);
                        // 防御 spin：未预期错误短暂 sleep
                        std::thread::sleep(Duration::from_millis(50));
                    }
                }
            }
        }
    });

    // ===== 主线程状态 =====
    let mut send_times: Vec<Option<Instant>> = vec![None; (max_hops as usize) + 1];
    let mut results: Vec<FastHop> = (1..=max_hops as u32)
        .map(|n| FastHop {
            hop_number: n,
            ip: None,
            rtt_ms: None,
        })
        .collect();
    let mut found_target_hop: Option<u8> = None;

    // ===== Send 阶段：每 TTL 一个独立 UdpSocket =====
    // 保留每 TTL 独立 src_port 的设计（每次 OS 分配随机源端口 → 五元组不同 →
    // ECMP hash 可能不同 → 有机会观察到骨干路径的多 IP 分岔）。
    //
    // Windows 上此循环整体耗时可能 1000ms+，但因 recv 线程独立运行,
    // 已到达的 ICMP 会在到达瞬间被打上正确的 recv_time,不会被 send 阻塞污染。
    let mut udp_sockets: Vec<UdpSocket> = Vec::with_capacity(max_hops as usize);
    for ttl in 1..=max_hops {
        let sock = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                log::warn!("UDP bind 失败 ttl={}: {}", ttl, e);
                continue;
            }
        };
        if let Err(e) = sock.set_ttl(ttl as u32) {
            log::warn!("UDP set_ttl({}) 失败: {}", ttl, e);
            continue;
        }

        let dst_port = BASE_PORT + ttl as u16;
        let dst: SocketAddr = SocketAddrV4::new(target, dst_port).into();

        send_times[ttl as usize] = Some(Instant::now());
        if let Err(e) = sock.send_to(&[0u8; 1], dst) {
            log::warn!("UDP send_to ttl={} dst={} failed: {}", ttl, dst, e);
        }
        udp_sockets.push(sock);

        // 顺手消化 channel 里已到达的响应（TTL 小的响应在本次 send 循环里可能就到了）
        while let Ok(evt) = rx.try_recv() {
            process_udp_event(
                evt,
                &send_times,
                &mut results,
                &mut found_target_hop,
                target,
                max_hops,
            );
        }
    }

    // ===== 等待阶段：继续处理响应直到 deadline 或提前满足 =====
    let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
    while Instant::now() < deadline {
        // 提前退出：找到目标跳且其前所有跳都有 IP
        if let Some(target_ttl) = found_target_hop {
            let all_done = (1..=target_ttl).all(|t| results[(t - 1) as usize].ip.is_some());
            if all_done {
                break;
            }
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        let wait = remaining.min(Duration::from_millis(50));
        match rx.recv_timeout(wait) {
            Ok(evt) => {
                process_udp_event(
                    evt,
                    &send_times,
                    &mut results,
                    &mut found_target_hop,
                    target,
                    max_hops,
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    // ===== 收尾 =====
    // 通知 recv 线程退出（read_timeout=50ms 决定最长退出等待）
    stop_flag.store(true, Ordering::Relaxed);
    let _ = recv_thread.join();

    // 若识别到目标跳,把目标跳 IP 补齐（如果尚未填入）并截断多余跳
    if let Some(target_ttl) = found_target_hop {
        let idx = (target_ttl - 1) as usize;
        if idx < results.len() && results[idx].ip.is_none() {
            let send_time = send_times[target_ttl as usize];
            // 这里没有对应的 recv_time,fallback 用 elapsed（通常不会走到,前面已经填过了）
            let rtt_ms = send_time.map(|t| t.elapsed().as_secs_f64() * 1000.0);
            results[idx].ip = Some(target.to_string());
            results[idx].rtt_ms = rtt_ms;
        }
        results.truncate(target_ttl as usize);
    }

    drop(udp_sockets);
    Ok(results)
}

/// 处理一条 recv 线程发来的 ICMP 事件：更新 results / found_target_hop。
/// RTT 计算 = evt.recv_time - send_times[ttl]，反映真实往返时间（不含 send 阶段耗时）。
fn process_udp_event(
    evt: RecvEvent,
    send_times: &[Option<Instant>],
    results: &mut [FastHop],
    found_target_hop: &mut Option<u8>,
    target: Ipv4Addr,
    max_hops: u8,
) {
    let ttl = evt.ttl;
    if ttl == 0 || ttl as usize > results.len() {
        return;
    }
    let from_target = evt.src_ip == target;

    log::debug!(
        "[udp-trace] icmp recv: type={} src={} reverse_ttl={} from_target={}",
        evt.icmp_type, evt.src_ip, ttl, from_target
    );

    if from_target {
        // 来自目标的回复：仅用于更新最小目标跳候选
        // 大 TTL 的包也能到目标,src_ip 都是 target,不能直接写入对应 hop 的 IP。
        let new_target_ttl = match *found_target_hop {
            Some(prev) => prev.min(ttl),
            None => ttl,
        };
        // 清掉 > new_target_ttl 的位置上误填的 target IP
        for clear_ttl in (new_target_ttl + 1)..=max_hops {
            let cidx = (clear_ttl - 1) as usize;
            if cidx < results.len() {
                if let Some(ref ip) = results[cidx].ip {
                    if *ip == target.to_string() {
                        results[cidx].ip = None;
                        results[cidx].rtt_ms = None;
                    }
                }
            }
        }
        let idx = (new_target_ttl - 1) as usize;
        if idx < results.len() {
            let send_time = send_times[new_target_ttl as usize];
            let rtt_ms = send_time.map(|t| (evt.recv_time - t).as_secs_f64() * 1000.0);
            results[idx].ip = Some(target.to_string());
            results[idx].rtt_ms = rtt_ms;
        }
        *found_target_hop = Some(new_target_ttl);
        return;
    }

    // 中间跳的 Time Exceeded / 少数 Dest Unreachable：填入对应 hop
    let idx = (ttl - 1) as usize;
    if results[idx].ip.is_some() {
        return; // 已有结果,忽略后续重复
    }
    let send_time = send_times[ttl as usize];
    let rtt_ms = send_time.map(|t| (evt.recv_time - t).as_secs_f64() * 1000.0);
    results[idx].ip = Some(evt.src_ip.to_string());
    results[idx].rtt_ms = rtt_ms;
}

/// 解析一个 ICMP 包，提取 (icmp_type, 原始 UDP 包对应的 TTL)。
///
/// IPv4 raw socket 收到的数据：[IP header 20+ bytes][ICMP data]
/// 我们关心两类：
/// - Time Exceeded(11)：来自中间路由器
/// - Destination Unreachable(3)：来自目标主机（Port Unreachable code=3）
///
/// 这两类 ICMP 在 ICMP body 后会带 8 字节"unused" + 原始 IP header(20+) + 原始
/// 传输层头 ≥8 字节。原始 UDP 头 8 字节包含：src_port(2) + dst_port(2) + len(2) + cksum(2)
/// 我们用 dst_port - BASE_PORT 反查 TTL。
fn parse_icmp_for_udp(data: &[u8], _target: Ipv4Addr) -> Option<(u8, u8)> {
    if data.len() < 28 {
        return None;
    }
    let ihl = (data[0] & 0x0F) as usize * 4;
    if data.len() < ihl + 8 {
        return None;
    }
    let icmp = &data[ihl..];
    let icmp_type = icmp[0];

    // 只处理 Time Exceeded 和 Destination Unreachable
    if icmp_type != 11 && icmp_type != 3 {
        return None;
    }

    // ICMP body: 4 bytes header(type/code/checksum) + 4 bytes unused + 原始 IP header + 原始 UDP header
    if icmp.len() < 8 + 20 + 8 {
        return None;
    }
    let inner_ip = &icmp[8..];
    // 内嵌 IP header
    if (inner_ip[0] >> 4) != 4 {
        return None; // 不是 IPv4，跳过
    }
    let inner_ihl = (inner_ip[0] & 0x0F) as usize * 4;
    if inner_ip.len() < inner_ihl + 8 {
        return None;
    }
    // 协议字段在 IP header offset 9（IPv4 protocol byte）
    let inner_proto = inner_ip[9];
    if inner_proto != 17 {
        // 17 = UDP
        return None;
    }
    let inner_udp = &inner_ip[inner_ihl..];
    if inner_udp.len() < 8 {
        return None;
    }
    let dst_port = u16::from_be_bytes([inner_udp[2], inner_udp[3]]);

    if dst_port < BASE_PORT {
        return None;
    }
    let ttl = dst_port - BASE_PORT;
    if ttl == 0 || ttl > 64 {
        return None;
    }
    Some((icmp_type, ttl as u8))
}
