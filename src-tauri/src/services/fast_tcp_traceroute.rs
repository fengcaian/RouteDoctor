// 并行 TCP traceroute（PingPlotter 风格 - 类似 tracetcp/scapy 的 SYN traceroute）
//
// 原理：
// - 为 TTL=1..max_hops 各开一个非阻塞 TCP socket，设置 IP_TTL=ttl，向目标的
//   target_port（默认 443）发起 connect。
// - 中间路由器返回 ICMP Time Exceeded(type=11)；其 ICMP body 内嵌的"原始 IP+
//   原始 TCP 头"前 8 字节包含：src_port(2) + dst_port(2) + seq(4)。我们通过
//   src_port 反查 TTL（每个 socket 绑定一个唯一源端口 BASE_PORT+ttl）。
// - 目标主机如果监听该端口 → 返回 SYN+ACK，TCP 三次握手成功 → connect 完成；
//   不监听 → 返回 RST，connect 失败但 errno=ECONNREFUSED；防火墙过滤
//   → connect 超时（落到我们的 deadline）。
// - 整体耗时 ≈ 1 个 timeout 窗口（与并行 ICMP 同级）。
//
// 优点：
// - 不依赖外部 tracetcp.exe（用户机器上未必安装）。
// - TCP 探测能穿透许多对 ICMP/UDP 过滤的防火墙，更接近"应用层可达性"。
//
// 限制：
// - 接收 ICMP Time Exceeded 仍需要 raw socket 权限（与 ICMP/UDP 并行一致）。
// - 目标跳的探测：用 connect 结果（成功 / ECONNREFUSED 都算到达）。

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};
use socket2::{Domain, Protocol, Socket, Type};
use crate::error::{AppError, AppResult};
use crate::services::fast_traceroute::FastHop;

/// TCP 源端口基址：每跳源端口 = BASE + ttl，方便回复时反查 TTL。
/// 选个高位、不冲突的临时端口范围。
const BASE_SRC_PORT: u16 = 36000;

/// 并行 TCP traceroute。target 必须是已解析的 IPv4 地址，target_port 通常 443。
pub async fn parallel_tcp_traceroute(
    target: Ipv4Addr,
    target_port: u16,
    max_hops: u32,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let max_hops = max_hops.clamp(1, 64) as u8;

    let icmp_socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))
        .map_err(|e| AppError::TracerouteError(format!(
            "无法创建 raw ICMP socket（TCP traceroute 需要管理员权限）: {}", e
        )))?;

    // 显式 bind 到 0.0.0.0:0：Windows 上 raw socket 在 recv_from 前必须先 bind
    // （见 fast_udp_traceroute.rs 里同样的注释；不 bind 会 WSAEINVAL=10022 spin）。
    let icmp_bind_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into();
    if let Err(e) = icmp_socket.bind(&icmp_bind_addr.into()) {
        return Err(AppError::TracerouteError(format!(
            "raw ICMP socket bind 失败: {}", e
        )));
    }

    icmp_socket.set_nonblocking(false).ok();
    icmp_socket
        .set_read_timeout(Some(Duration::from_millis(50)))
        .ok();

    let result = tokio::task::spawn_blocking(move || -> AppResult<Vec<FastHop>> {
        do_tcp_traceroute_blocking(icmp_socket, target, target_port, max_hops, timeout_ms)
    })
    .await
    .map_err(|e| AppError::TracerouteError(format!("TCP traceroute task join 失败: {}", e)))??;

    Ok(result)
}

fn do_tcp_traceroute_blocking(
    icmp_socket: Socket,
    target: Ipv4Addr,
    target_port: u16,
    max_hops: u8,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let mut send_times: Vec<Option<Instant>> = vec![None; (max_hops as usize) + 1];
    let mut results: Vec<FastHop> = (1..=max_hops as u32)
        .map(|n| FastHop {
            hop_number: n,
            ip: None,
            rtt_ms: None,
        })
        .collect();

    // 每个 TTL 一个 TCP socket，保存以便后续轮询连接状态
    let mut tcp_sockets: Vec<(u8, Socket, u16)> = Vec::with_capacity(max_hops as usize); // (ttl, socket, src_port)

    let dst: SocketAddr = SocketAddrV4::new(target, target_port).into();

    // 1) 顺序发出所有 TTL 的 TCP SYN
    for ttl in 1..=max_hops {
        let sock = match Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("TCP socket 创建失败 ttl={}: {}", ttl, e);
                continue;
            }
        };

        // 绑定到固定源端口，便于通过 ICMP 回复中的内嵌 TCP 头反查 TTL
        let src_port = BASE_SRC_PORT.saturating_add(ttl as u16);
        let bind_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, src_port).into();
        if let Err(e) = sock.bind(&bind_addr.into()) {
            // 端口冲突就让系统自动选；这种情况下我们就无法用 src_port 反查，
            // 但仍可记录 socket 与 ttl 的映射在内存表里。
            log::debug!("TCP bind src_port={} 失败（{}），改为系统分配", src_port, e);
            let any_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into();
            if let Err(e2) = sock.bind(&any_addr.into()) {
                log::warn!("TCP bind any 也失败 ttl={}: {}", ttl, e2);
                continue;
            }
        }

        if let Err(e) = sock.set_ttl(ttl as u32) {
            log::warn!("TCP set_ttl({}) 失败: {}", ttl, e);
            continue;
        }
        // 非阻塞 connect：发出 SYN 后立即返回，后续用 select/getsockopt 检查
        if let Err(e) = sock.set_nonblocking(true) {
            log::warn!("TCP set_nonblocking 失败 ttl={}: {}", ttl, e);
            continue;
        }

        send_times[ttl as usize] = Some(Instant::now());
        let actual_src_port = sock
            .local_addr()
            .ok()
            .and_then(|a| a.as_socket().map(|s| s.port()))
            .unwrap_or(src_port);

        // 非阻塞 connect 通常立即返回 WouldBlock / InProgress，这是预期行为
        match sock.connect(&dst.into()) {
            Ok(_) => {} // 极少见的同步成功（loopback / 已就绪）
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.raw_os_error() == Some(115)  // EINPROGRESS (Linux)
                    || e.raw_os_error() == Some(10035) // WSAEWOULDBLOCK (Windows)
                => {}
            Err(e) => {
                log::debug!("TCP connect ttl={} 同步错误: {}", ttl, e);
            }
        }

        tcp_sockets.push((ttl, sock, actual_src_port));
        std::thread::sleep(Duration::from_millis(2));
    }

    // 2) 在 timeout 窗口内：
    //    a) 持续接收 ICMP Time Exceeded（中间跳）
    //    b) 轮询 TCP socket 状态（目标跳：connect 成功或 ECONNREFUSED 都算到达）
    let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
    let mut buf = [std::mem::MaybeUninit::new(0u8); 1500];
    let mut found_target_hop: Option<u8> = None;

    while Instant::now() < deadline {
        // 提前退出：找到目标 + 之前所有跳都有结果
        if let Some(target_ttl) = found_target_hop {
            let all_done = (1..=target_ttl).all(|t| results[(t - 1) as usize].ip.is_some());
            if all_done {
                break;
            }
        }

        // a) 尝试接收 ICMP（read_timeout 50ms 触发周期性返回）
        match icmp_socket.recv_from(&mut buf) {
            Ok((len, src)) => {
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

                // 通过 ICMP body 内嵌 TCP 头的 src_port 反查 TTL
                if let Some(ttl) = parse_icmp_for_tcp(data, &tcp_sockets) {
                    let idx = (ttl - 1) as usize;
                    if idx < results.len() && results[idx].ip.is_none() {
                        let send_time = send_times[ttl as usize];
                        let rtt_ms = send_time.map(|t| t.elapsed().as_secs_f64() * 1000.0);
                        results[idx].ip = Some(src_ip.to_string());
                        results[idx].rtt_ms = rtt_ms;
                    }
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::WouldBlock
                    && e.kind() != std::io::ErrorKind::TimedOut
                {
                    log::warn!("TCP traceroute icmp recv error: {}", e);
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }

        // b) 轮询 TCP sockets：connect 完成（成功 / ECONNREFUSED）→ 该 TTL 包到达了目标。
        //
        // 关键修正：TTL 大于"真正目标跳"的 socket 也都会成功连上目标，因为路径长度固定、
        // TTL 余量更多。如果直接把这些 hop 的 IP 写成 target，就会出现"30 跳全是目标 IP"。
        // 因此：所有"connect 成功"的 socket 只用来更新"最小目标跳"候选；中间跳的 IP
        // 只能来自 a) 步骤里 ICMP Time Exceeded 的 src（中间路由器）。
        for (ttl, sock, _src_port) in tcp_sockets.iter() {
            if let Some(reached) = check_tcp_connect_done(sock) {
                if reached {
                    let new_target_ttl = match found_target_hop {
                        Some(prev) => prev.min(*ttl),
                        None => *ttl,
                    };
                    // 把"最小目标跳"那一行立即写为 target IP（让 all_done 提前退出生效）。
                    // 比 new_target_ttl 大的 socket 即使也成功连上目标，也不写——它们
                    // 不是"真正的目标跳"，对应 hop 的真实路由器应来自 ICMP Time Exceeded。
                    let idx = (new_target_ttl - 1) as usize;
                    if idx < results.len() && results[idx].ip.is_none() {
                        let send_time = send_times[new_target_ttl as usize];
                        let rtt_ms = send_time.map(|t| t.elapsed().as_secs_f64() * 1000.0);
                        results[idx].ip = Some(target.to_string());
                        results[idx].rtt_ms = rtt_ms;
                    }
                    found_target_hop = Some(new_target_ttl);
                }
            }
        }
    }

    // 3) 截断后续不必要的跳，并填好目标跳的 IP/RTT
    if let Some(target_ttl) = found_target_hop {
        let idx = (target_ttl - 1) as usize;
        if idx < results.len() && results[idx].ip.is_none() {
            let send_time = send_times[target_ttl as usize];
            let rtt_ms = send_time.map(|t| t.elapsed().as_secs_f64() * 1000.0);
            results[idx].ip = Some(target.to_string());
            results[idx].rtt_ms = rtt_ms;
        }
        results.truncate(target_ttl as usize);
    }

    drop(tcp_sockets);
    Ok(results)
}

/// 检查非阻塞 TCP connect 是否完成，返回 Some(true)=到达目标，Some(false)=明确失败但
/// 不是因为到达目标，None=仍在进行中。
///
/// 到达目标的两个信号：
/// - 连接成功（getsockopt(SO_ERROR)==0 且 socket 可写）
/// - ECONNREFUSED（端口未监听，但目标主机响应了 RST，证明可达）
fn check_tcp_connect_done(sock: &Socket) -> Option<bool> {
    match sock.take_error() {
        Ok(None) => {
            // 没有错误：尝试通过 peer_addr 判断是否已连接
            match sock.peer_addr() {
                Ok(_) => Some(true), // connect 成功 = 到达目标
                Err(_) => None,       // 还在 in-progress
            }
        }
        Ok(Some(err)) => {
            // ECONNREFUSED：目标主机回了 RST，可达
            // Windows: WSAECONNREFUSED = 10061；Linux: ECONNREFUSED = 111
            let raw = err.raw_os_error();
            if raw == Some(10061) || raw == Some(111) {
                return Some(true);
            }
            // 其它错误（EHOSTUNREACH/ENETUNREACH）= 中间跳的 ICMP 反馈被 OS 转发回来；
            // 我们已经从 raw socket 收到了 ICMP，此处忽略。
            Some(false)
        }
        Err(_) => None,
    }
}

/// 解析 ICMP 包，提取这是哪一跳的 TCP SYN 触发的 Time Exceeded/Destination Unreachable。
/// 通过 ICMP body 内嵌的"原始 IP + 原始 TCP 头"前 4 字节（src_port + dst_port）反查。
///
/// 返回 Some(ttl) 当：
///   - 是 Time Exceeded(11) 或 Destination Unreachable(3)
///   - 内嵌协议是 TCP(6)
///   - 内嵌 src_port 在我们记录的 (ttl, src_port) 表中
fn parse_icmp_for_tcp(data: &[u8], tcp_sockets: &[(u8, Socket, u16)]) -> Option<u8> {
    if data.len() < 28 {
        return None;
    }
    let ihl = (data[0] & 0x0F) as usize * 4;
    if data.len() < ihl + 8 {
        return None;
    }
    let icmp = &data[ihl..];
    let icmp_type = icmp[0];
    if icmp_type != 11 && icmp_type != 3 {
        return None;
    }

    if icmp.len() < 8 + 20 + 4 {
        return None;
    }
    let inner_ip = &icmp[8..];
    if (inner_ip[0] >> 4) != 4 {
        return None;
    }
    let inner_ihl = (inner_ip[0] & 0x0F) as usize * 4;
    if inner_ip.len() < inner_ihl + 4 {
        return None;
    }
    let inner_proto = inner_ip[9];
    if inner_proto != 6 {
        // 6 = TCP
        return None;
    }
    let inner_tcp = &inner_ip[inner_ihl..];
    if inner_tcp.len() < 4 {
        return None;
    }
    let src_port = u16::from_be_bytes([inner_tcp[0], inner_tcp[1]]);

    // 在表里查 (ttl, src_port) 反查
    for (ttl, _sock, sp) in tcp_sockets {
        if *sp == src_port {
            return Some(*ttl);
        }
    }
    None
}
