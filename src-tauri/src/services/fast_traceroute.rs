// 并行 ICMP traceroute（PingPlotter 风格）
//
// 原理：同时给 TTL=1..max_hops 发 30 个 ICMP echo request 包，每个包带唯一
// (identifier, sequence)。中间路由器回 Time Exceeded（type=11），目标主机回
// Echo Reply（type=0）。一次性收到所有响应，整体耗时 ≈ 1 个 timeout 窗口。
//
// 为什么不用 surge-ping：surge-ping 0.8 不暴露 TTL 设置和 TimeExceeded 处理。
// 为什么不用系统 tracert.exe：串行探测，30 跳里有几个超时跳就会拖到 20-60 秒。
//
// 平台限制：
// - Linux/macOS：raw ICMP socket 工作正常（部分发行版需要 cap_net_raw 或 root）。
// - Windows：用户态 raw socket 即使管理员也收不到中间跳的 Time Exceeded
//   （Windows 内核投递限制），所以本文件 Windows 分支转发到
//   `services::win_icmp_traceroute`，那里走 IcmpSendEcho2 API。
// 失败时回退由调用方处理。

use std::net::Ipv4Addr;

#[cfg(not(windows))]
use std::net::{IpAddr, SocketAddr, SocketAddrV4};
#[cfg(not(windows))]
use std::time::{Duration, Instant};
#[cfg(not(windows))]
use socket2::{Domain, Protocol, Socket, Type};
#[cfg(not(windows))]
use tokio::sync::mpsc;

use crate::error::AppResult;
#[cfg(not(windows))]
use crate::error::AppError;

/// 单跳探测结果
#[derive(Debug, Clone)]
pub struct FastHop {
    pub hop_number: u32,
    pub ip: Option<String>,    // None 表示该跳超时无响应
    pub rtt_ms: Option<f64>,
}

/// 并行 ICMP traceroute。target 必须是已解析的 IPv4 地址。
/// 整体耗时 = max(网络最大 RTT, timeout_ms)
///
/// 平台分发：
/// - Windows: 走 `IcmpSendEcho2`（iphlpapi.dll），内核 ICMP 引擎处理 TTL +
///   Time Exceeded，能拿到完整中间跳，无需管理员，也无需 Npcap。
/// - Linux/macOS: 走 raw ICMP socket 自己解析 ICMP 回包。
pub async fn parallel_icmp_traceroute(
    target: Ipv4Addr,
    max_hops: u32,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    #[cfg(windows)]
    {
        return crate::services::win_icmp_traceroute::parallel_icmp_traceroute(
            target, max_hops, timeout_ms,
        )
        .await;
    }
    #[cfg(not(windows))]
    {
        parallel_icmp_traceroute_rawsocket(target, max_hops, timeout_ms).await
    }
}

/// raw socket 版的 ICMP traceroute（Linux/macOS 上使用）。
/// Windows 上理论上可以建 raw socket，但内核不会可靠投递 Time Exceeded，所以不走这条。
#[cfg(not(windows))]
async fn parallel_icmp_traceroute_rawsocket(
    target: Ipv4Addr,
    max_hops: u32,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let max_hops = max_hops.clamp(1, 64) as u8;

    // 创建一个共享的 raw ICMP socket。Linux/Windows 都需要权限。
    let socket = Socket::new(Domain::IPV4, Type::RAW, Some(Protocol::ICMPV4))
        .map_err(|e| AppError::TracerouteError(format!("无法创建 raw ICMP socket（可能需要管理员权限）: {}", e)))?;

    socket.set_nonblocking(true)
        .map_err(|e| AppError::TracerouteError(format!("set_nonblocking 失败: {}", e)))?;

    // 用 process pid 低 16 位做 identifier，减少不同进程的回复混淆
    let identifier = (std::process::id() & 0xFFFF) as u16;

    // 把 socket 转给 tokio 用：socket2 → std → tokio std-style 不直接兼容，
    // 这里用最简单的策略：tokio::task::spawn_blocking 跑 send/recv，
    // 通过 channel 把结果送回来。
    //
    // 但为了简单和 latency 准确，我们采用混合方案：
    // - 主任务并行 spawn N 个 send（每个 TTL 一个）
    // - 单个接收 task 在 timeout 窗口内持续 recv，把每个 reply 通过 channel 发出
    // - 主任务汇总

    let target_addr: SocketAddr = SocketAddrV4::new(target, 0).into();

    // 由于 socket2 socket 没有原生 async，我们把它放在阻塞任务中。
    // 这里直接用同步阻塞模式（在 spawn_blocking 里），性能足够（30 跳的小循环）。
    let result = tokio::task::spawn_blocking(move || -> AppResult<Vec<FastHop>> {
        do_traceroute_blocking(socket, target, target_addr, identifier, max_hops, timeout_ms)
    })
    .await
    .map_err(|e| AppError::TracerouteError(format!("traceroute task join 失败: {}", e)))??;

    Ok(result)
}

#[cfg(not(windows))]
fn do_traceroute_blocking(
    socket: Socket,
    target: Ipv4Addr,
    target_addr: SocketAddr,
    identifier: u16,
    max_hops: u8,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    // 设为阻塞，但用 read_timeout 控制最长等待。
    socket.set_nonblocking(false).ok();
    socket.set_read_timeout(Some(Duration::from_millis(50))).ok();

    // 记录每个 seq 对应的 TTL（即跳号）和发送时间
    let mut send_times: Vec<Option<Instant>> = vec![None; (max_hops as usize) + 1];
    // 每跳的结果（按 hop_number 索引，0 不用）
    let mut results: Vec<FastHop> = (1..=max_hops as u32)
        .map(|n| FastHop { hop_number: n, ip: None, rtt_ms: None })
        .collect();

    // 1) 顺序发出所有 TTL 包（每包间隔几毫秒避免突发）
    for ttl in 1..=max_hops {
        socket.set_ttl(ttl as u32)
            .map_err(|e| AppError::TracerouteError(format!("set_ttl({}) 失败: {}", ttl, e)))?;

        let seq = ttl as u16; // seq = ttl，方便回复时反查
        let packet = build_icmp_echo_request(identifier, seq);
        send_times[ttl as usize] = Some(Instant::now());
        if let Err(e) = socket.send_to(&packet, &target_addr.into()) {
            log::warn!("send_to ttl={} failed: {}", ttl, e);
        }
        // 微小间隔，避免对端速率限制
        std::thread::sleep(Duration::from_millis(2));
    }

    // 2) 接收循环：在 timeout 窗口内持续接收
    let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
    let mut buf = [std::mem::MaybeUninit::new(0u8); 1500];
    let mut found_target_hop: Option<u8> = None;

    while Instant::now() < deadline {
        // 已经收到目标回复且其后所有跳都已经处理 → 提前退出
        if let Some(target_ttl) = found_target_hop {
            // 是否所有 ttl <= target_ttl 的跳都有结果了
            let all_done = (1..=target_ttl).all(|t| results[(t - 1) as usize].ip.is_some());
            if all_done { break; }
        }

        match socket.recv_from(&mut buf) {
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

                // 解析 ICMP 包
                if let Some((reply_type, original_seq)) = parse_icmp_reply(data, identifier) {
                    let ttl = original_seq as u8;
                    if ttl == 0 || ttl as usize > results.len() { continue; }

                    // 关键修正：并行 traceroute 中，所有 ttl ≥ 真目标跳的 echo
                    // 都会到达目标，目标对每一个都回 Echo Reply。如果按 seq=ttl
                    // 反查直接写入对应 hop，会把目标 IP 错填到第 N+1..max_hops 跳上
                    // （表现为后段 IP 全部相同）。
                    // 因此：来自目标的 Echo Reply（type=0 && src==target）只用来
                    // 更新"最小目标跳"候选，绝不写到中间 hop；中间跳的 IP 只能来自
                    // type=11 Time Exceeded（src 是中间路由器）。
                    let from_target = reply_type == 0 && src_ip == target;
                    if from_target {
                        let new_target_ttl = match found_target_hop {
                            Some(prev) => prev.min(ttl),
                            None => ttl,
                        };

                        // 防御性清理：清掉所有 ttl > new_target_ttl 上误填的目标 IP
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

                        // 把目标跳那一行 IP 立即写为 target，让 all_done 判断生效
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

                    // 非目标回复（主要是 type=11 Time Exceeded）：正常填入对应 hop
                    let idx = (ttl - 1) as usize;
                    if results[idx].ip.is_some() { continue; } // 已有结果，跳过重复回复

                    let send_time = send_times[ttl as usize];
                    let rtt_ms = send_time.map(|t| t.elapsed().as_secs_f64() * 1000.0);

                    results[idx].ip = Some(src_ip.to_string());
                    results[idx].rtt_ms = rtt_ms;
                }
            }
            Err(e) => {
                // WouldBlock / TimedOut 都是正常的（read_timeout 50ms 到了）
                if e.kind() != std::io::ErrorKind::WouldBlock
                    && e.kind() != std::io::ErrorKind::TimedOut {
                    log::warn!("recv_from error: {}", e);
                }
            }
        }
    }

    // 3) 截断：如果识别到目标主机的跳，丢弃后面没必要的跳
    if let Some(target_ttl) = found_target_hop {
        results.truncate(target_ttl as usize);
    }

    Ok(results)
}

/// 构造一个最小的 ICMP echo request 包
/// 格式：
///   type(1) | code(1) | checksum(2) | identifier(2) | sequence(2) | payload
#[cfg(not(windows))]
fn build_icmp_echo_request(identifier: u16, sequence: u16) -> Vec<u8> {
    let mut pkt = vec![0u8; 16];
    pkt[0] = 8;           // type = 8 (Echo Request)
    pkt[1] = 0;           // code = 0
    pkt[2] = 0;           // checksum 占位
    pkt[3] = 0;
    pkt[4..6].copy_from_slice(&identifier.to_be_bytes());
    pkt[6..8].copy_from_slice(&sequence.to_be_bytes());
    // payload 8 字节（保持包大小 ≥ 16 与 system ping 兼容）
    for i in 8..16 {
        pkt[i] = (i - 8) as u8;
    }
    let cksum = icmp_checksum(&pkt);
    pkt[2..4].copy_from_slice(&cksum.to_be_bytes());
    pkt
}

#[cfg(not(windows))]
fn icmp_checksum(data: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < data.len() {
        sum += u16::from_be_bytes([data[i], data[i + 1]]) as u32;
        i += 2;
    }
    if i < data.len() {
        sum += (data[i] as u32) << 8;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xFFFF) + (sum >> 16);
    }
    !(sum as u16)
}

/// 解析一个收到的 ICMP 包，返回 (reply_type, 原始 echo 包的 sequence)。
/// 如果不是我们感兴趣的回复类型则返回 None。
///
/// 收到的 IPv4 raw socket 数据包：[IP header 20+ bytes][ICMP data]
/// - Echo Reply (type=0)：ICMP header 直接带 identifier + sequence
/// - Time Exceeded (type=11)：ICMP header 后面带 8 bytes 未使用 + 原始 IP header(20) + 原始 ICMP header(8)
///   原始 ICMP header 里有我们发的 identifier 和 sequence
#[cfg(not(windows))]
fn parse_icmp_reply(data: &[u8], expected_id: u16) -> Option<(u8, u16)> {
    if data.len() < 28 { return None; }
    // IPv4 header 长度
    let ihl = (data[0] & 0x0F) as usize * 4;
    if data.len() < ihl + 8 { return None; }
    let icmp = &data[ihl..];
    let icmp_type = icmp[0];

    match icmp_type {
        0 => {
            // Echo Reply - identifier and seq directly in ICMP header
            if icmp.len() < 8 { return None; }
            let id = u16::from_be_bytes([icmp[4], icmp[5]]);
            let seq = u16::from_be_bytes([icmp[6], icmp[7]]);
            if id != expected_id { return None; }
            Some((0, seq))
        }
        11 | 3 => {
            // Time Exceeded (11) or Destination Unreachable (3)
            // ICMP body: 4 bytes unused + original IP header + original ICMP (≥8 bytes)
            if icmp.len() < 8 + 20 + 8 { return None; }
            let inner_ihl = (icmp[8] & 0x0F) as usize * 4;
            let inner_icmp_offset = 8 + inner_ihl;
            if icmp.len() < inner_icmp_offset + 8 { return None; }
            let inner = &icmp[inner_icmp_offset..];
            let id = u16::from_be_bytes([inner[4], inner[5]]);
            let seq = u16::from_be_bytes([inner[6], inner[7]]);
            if id != expected_id { return None; }
            Some((icmp_type, seq))
        }
        _ => None,
    }
}

// 让 compiler 不抱怨未使用的 mpsc（保留以备未来流式版本）
#[cfg(not(windows))]
#[allow(dead_code)]
fn _unused_mpsc<T>() -> mpsc::Sender<T> {
    let (tx, _) = mpsc::channel(1);
    tx
}
