// 并行 UDP traceroute（PingPlotter / mtr 风格）
//
// 原理：
// - 为 TTL=1..max_hops 各开一个 UDP socket，设置 IP_TTL=ttl，向目标的端口
//   33434+ttl 发送一个最小数据包（标准 traceroute 使用的端口范围）。
// - 中间路由器返回 ICMP Time Exceeded(type=11)；目标主机不监听这些端口，
//   返回 ICMP Destination Unreachable(type=3, code=3, Port Unreachable)。
// - 我们用一个 raw ICMP socket 在 timeout 窗口内统一接收所有 ICMP 回复，
//   通过解析 ICMP 包内嵌的"原始 IP+UDP 头"提取 dst_port，反查 ttl =
//   dst_port - 33434。
// - 整体耗时 ≈ 1 个 timeout 窗口（与并行 ICMP 同级）。
//
// 优点：
// - 不依赖系统 tracert/traceroute 串行命令，30 跳里有几个超时跳也只等一次。
// - UDP 比 ICMP 在某些 ISP 路径上回复更稳定（不会被 ICMP 速率限制误伤）。
//
// 限制：
// - 接收 ICMP 仍需要 raw socket 权限（Windows 通常 Tauri 应用可用，失败时回退）。

use std::net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::time::{Duration, Instant};
use socket2::{Domain, Protocol, Socket, Type};
use crate::error::{AppError, AppResult};
use crate::services::fast_traceroute::FastHop;

/// 标准 traceroute 起始端口，与 Linux/BSD 的 traceroute 默认一致。
/// 我们用 BASE+ttl 来反查响应对应的 TTL。
const BASE_PORT: u16 = 33434;

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
    // 在 fast_traceroute.rs(ICMP) 里 send_to 会触发隐式 bind，所以那里没暴露这个问题；
    // 这里 raw ICMP socket 只用于 recv，必须显式 bind。
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
    // 每跳的发送时间，用于计算 RTT；索引 0 不用
    let mut send_times: Vec<Option<Instant>> = vec![None; (max_hops as usize) + 1];
    // 每跳的结果（按 hop_number 索引到位置 hop-1）
    let mut results: Vec<FastHop> = (1..=max_hops as u32)
        .map(|n| FastHop {
            hop_number: n,
            ip: None,
            rtt_ms: None,
        })
        .collect();

    // 1) 顺序发出所有 TTL 的 UDP 包
    //
    // 用标准 UdpSocket 即可：每个 socket 绑定 0.0.0.0:0，设置 ttl，sendto 到
    // (target, BASE_PORT+ttl)。我们不关心源端口反查（dst_port 已经能反查 TTL）。
    // 保留 socket 在 vec 中防止过早 drop。
    let mut udp_sockets: Vec<UdpSocket> = Vec::with_capacity(max_hops as usize);
    for ttl in 1..=max_hops {
        let sock = UdpSocket::bind("0.0.0.0:0")
            .map_err(|e| AppError::TracerouteError(format!("UDP bind 失败 ttl={}: {}", ttl, e)))?;
        sock.set_ttl(ttl as u32)
            .map_err(|e| AppError::TracerouteError(format!("UDP set_ttl({}) 失败: {}", ttl, e)))?;

        let dst_port = BASE_PORT + ttl as u16;
        let dst: SocketAddr = SocketAddrV4::new(target, dst_port).into();

        send_times[ttl as usize] = Some(Instant::now());

        // 一个最小载荷即可（≥1 字节，路由器不需要载荷内容）
        if let Err(e) = sock.send_to(&[0u8; 1], dst) {
            log::warn!("UDP send_to ttl={} dst={} failed: {}", ttl, dst, e);
        }

        udp_sockets.push(sock);
        // 微小间隔避免突发被速率限制
        std::thread::sleep(Duration::from_millis(2));
    }

    // 2) 在 timeout 窗口内持续接收 ICMP 回复
    let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
    let mut buf = [std::mem::MaybeUninit::new(0u8); 1500];
    let mut found_target_hop: Option<u8> = None;

    while Instant::now() < deadline {
        // 提前退出：已找到目标跳，且其前面所有跳都有了结果
        if let Some(target_ttl) = found_target_hop {
            let all_done = (1..=target_ttl).all(|t| results[(t - 1) as usize].ip.is_some());
            if all_done {
                break;
            }
        }

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

                // 解析 ICMP 包，提取这是哪个 TTL 的回复以及类型
                if let Some((reply_type, ttl)) = parse_icmp_for_udp(data, target) {
                    if ttl == 0 || ttl as usize > results.len() {
                        continue;
                    }

                    // 判断这条回复是否来自目标
                    let from_target = src_ip == target;

                    // 诊断日志：每条 ICMP 回复都打一条，便于排查
                    log::debug!(
                        "[udp-trace] icmp recv: type={} src={} reverse_ttl={} from_target={}",
                        reply_type, src_ip, ttl, from_target
                    );

                    // 关键修正：UDP traceroute 中，一旦某个 TTL 足够到达目标，
                    // 比它大的 TTL 包也都会到目标（路径长度固定，TTL 余量更多），
                    // 目标对它们全部回 ICMP Port Unreachable，src_ip 都是 target。
                    // 因此 src_ip == target 的回复**不能**直接写入对应 hop —— 它们只能
                    // 用来确定"最小的目标跳 TTL"。中间跳的 IP 只能来自 type=11
                    // (Time Exceeded)，src 是中间路由器（!= target）。
                    if from_target {
                        // 来自目标的回复（不论 type=3 还是别的）：更新最小目标跳候选，
                        // 把目标跳那一行的 IP 立刻写为 target（用于提前退出判断）。
                        let new_target_ttl = match found_target_hop {
                            Some(prev) => prev.min(ttl),
                            None => ttl,
                        };

                        // 防御性清理：如果 new_target_ttl 比当前已写入的某些 hop 小，
                        // 那些更大 ttl 的 hop 上误填的 target IP 必须清掉
                        // （走到这里是因为之前可能某些回复先到顺序乱）
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
                            let rtt_ms = send_time.map(|t| t.elapsed().as_secs_f64() * 1000.0);
                            results[idx].ip = Some(target.to_string());
                            results[idx].rtt_ms = rtt_ms;
                        }
                        found_target_hop = Some(new_target_ttl);
                        continue;
                    }

                    // 其它情况（主要是 type=11 中间跳的 Time Exceeded，
                    // 或极少数中间路由器返回 type=3 host/net unreachable）：
                    // 正常填入对应 hop 的 IP。
                    let idx = (ttl - 1) as usize;
                    if results[idx].ip.is_some() {
                        continue; // 已有结果，忽略重复
                    }
                    let send_time = send_times[ttl as usize];
                    let rtt_ms = send_time.map(|t| t.elapsed().as_secs_f64() * 1000.0);

                    results[idx].ip = Some(src_ip.to_string());
                    results[idx].rtt_ms = rtt_ms;
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::WouldBlock
                    && e.kind() != std::io::ErrorKind::TimedOut
                {
                    log::warn!("UDP traceroute icmp recv error: {}", e);
                    // 防御 spin：未预期的错误（如某些平台上的 WSAEINVAL）也短暂 sleep
                    std::thread::sleep(Duration::from_millis(50));
                }
            }
        }
    }

    // 3) 截断后续不必要的跳，并把目标跳的 IP/RTT 填好
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

    // udp_sockets 在此 drop，回收所有 UDP socket
    drop(udp_sockets);

    Ok(results)
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
