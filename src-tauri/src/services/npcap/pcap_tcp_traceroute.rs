// 通过 Npcap 实现真正的并行 TCP traceroute（仅 Windows）
//
// 与 services::fast_tcp_traceroute 的核心差别：
// - fast_tcp_traceroute 用标准 TCP socket connect + raw ICMP socket 收 ICMP
//   → Windows 内核会过滤掉由本机 TCP 触发的 ICMP Time Exceeded → 中间跳收不到
// - 本实现用 pcap 抓所有 ICMP（绕过过滤）+ 标准 TCP socket 发 SYN
//   → 真正能拿到中间跳路由器的 IP
//
// 流程跟 pcap_udp_traceroute 一致，只是发 TCP SYN 替代发 UDP，
// 且通过 ICMP body 内嵌的 TCP 头的 src_port 反查 TTL（每跳源端口 = BASE_SRC_PORT + ttl）

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::{Duration, Instant};

use pcap::{Capture, Device};
use pnet_packet::Packet;
use pnet_packet::ipv4::Ipv4Packet;
use pnet_packet::ip::IpNextHeaderProtocols;
use pnet_packet::icmp::IcmpPacket;
use socket2::{Domain, Protocol, Socket, Type};

use crate::error::{AppError, AppResult};
use crate::services::fast_traceroute::FastHop;

/// TCP 源端口基址：每跳源端口 = BASE + ttl，便于通过 ICMP 内嵌 TCP 头反查 TTL
const BASE_SRC_PORT: u16 = 36000;

/// 通过 Npcap 实现的并行 TCP traceroute。target 必须是已解析的 IPv4，target_port 通常 443。
pub async fn parallel_tcp_traceroute(
    target: Ipv4Addr,
    target_port: u16,
    max_hops: u32,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let max_hops = max_hops.clamp(1, 64) as u8;

    let result = tokio::task::spawn_blocking(move || -> AppResult<Vec<FastHop>> {
        do_pcap_tcp_traceroute(target, target_port, max_hops, timeout_ms)
    })
    .await
    .map_err(|e| AppError::TracerouteError(format!("pcap TCP traceroute task join 失败: {}", e)))??;

    Ok(result)
}

fn do_pcap_tcp_traceroute(
    target: Ipv4Addr,
    target_port: u16,
    max_hops: u8,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let device = pick_default_device()?;
    log::info!(
        "[pcap-tcp] 使用网卡 {} ({})",
        device.name,
        device.desc.as_deref().unwrap_or("无描述")
    );

    let mut cap = Capture::from_device(device.clone())
        .map_err(|e| AppError::TracerouteError(format!("pcap from_device 失败: {}", e)))?
        .promisc(false)
        .immediate_mode(true)
        .timeout(50)
        .snaplen(1500)
        .open()
        .map_err(|e| AppError::TracerouteError(format!("pcap open 失败（Npcap 是否已启动？）: {}", e)))?;

    // BPF 过滤：抓 ICMP（中间跳的 Time Exceeded）+ 来自目标的 TCP（目标的 SYN-ACK/RST）
    // 这样我们既能识别中间跳，也能从抓到的 SYN-ACK 反查"是哪个 ttl 的探测到达了目标"。
    let bpf = format!("icmp or (tcp and src host {} and src port {})", target, target_port);
    if let Err(e) = cap.filter(&bpf, true) {
        log::warn!("[pcap-tcp] 设置 BPF 过滤失败（{}），将抓全部包并自行过滤", e);
    } else {
        log::info!("[pcap-tcp] BPF 过滤: {}", bpf);
    }

    let datalink = cap.get_datalink();

    let mut send_times: Vec<Option<Instant>> = vec![None; (max_hops as usize) + 1];
    let mut results: Vec<FastHop> = (1..=max_hops as u32)
        .map(|n| FastHop {
            hop_number: n,
            ip: None,
            rtt_ms: None,
        })
        .collect();

    // 发包：每跳一个非阻塞 TCP socket，源端口固定为 BASE+ttl
    let dst: SocketAddr = SocketAddrV4::new(target, target_port).into();
    let mut tcp_sockets: Vec<(u8, Socket)> = Vec::with_capacity(max_hops as usize);

    for ttl in 1..=max_hops {
        let sock = match Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)) {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[pcap-tcp] socket 创建失败 ttl={}: {}", ttl, e);
                continue;
            }
        };

        let src_port = BASE_SRC_PORT.saturating_add(ttl as u16);
        let bind_addr: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, src_port).into();
        if sock.bind(&bind_addr.into()).is_err() {
            // 源端口冲突就让 OS 自动分配；丢失了"用 src_port 反查 ttl"的能力
            // → 解析时退回到通过 socket 列表查找
            let any: SocketAddr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into();
            if let Err(e) = sock.bind(&any.into()) {
                log::warn!("[pcap-tcp] bind 完全失败 ttl={}: {}", ttl, e);
                continue;
            }
        }

        if sock.set_ttl(ttl as u32).is_err() {
            continue;
        }
        if sock.set_nonblocking(true).is_err() {
            continue;
        }

        send_times[ttl as usize] = Some(Instant::now());
        match sock.connect(&dst.into()) {
            Ok(_) => {}
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.raw_os_error() == Some(115)
                    || e.raw_os_error() == Some(10035) => {}
            Err(e) => {
                log::debug!("[pcap-tcp] connect ttl={} 同步错误: {}", ttl, e);
            }
        }

        tcp_sockets.push((ttl, sock));
        std::thread::sleep(Duration::from_millis(2));
    }

    // 抓包循环 + 轮询 TCP socket 状态
    let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
    let mut found_target_hop: Option<u8> = None;

    while Instant::now() < deadline {
        if let Some(target_ttl) = found_target_hop {
            let all_done = (1..=target_ttl).all(|t| results[(t - 1) as usize].ip.is_some());
            if all_done {
                break;
            }
        }

        // 抓 pcap 数据：可能是 ICMP（中间跳）或来自目标的 TCP（SYN-ACK/RST）
        match cap.next_packet() {
            Ok(packet) => {
                // 优先尝试解析为 ICMP
                if let Some((src_ip, ttl)) = parse_icmp_for_tcp(packet.data, datalink) {
                    if ttl == 0 || ttl as usize > results.len() {
                        continue;
                    }
                    let from_target = src_ip == target;
                    log::debug!(
                        "[pcap-tcp] icmp recv: src={} reverse_ttl={} from_target={}",
                        src_ip, ttl, from_target
                    );

                    if from_target {
                        update_target_hop(&mut found_target_hop, ttl, max_hops, target, &send_times, &mut results);
                        continue;
                    }

                    let idx = (ttl - 1) as usize;
                    if results[idx].ip.is_none() {
                        let send_time = send_times[ttl as usize];
                        let rtt_ms = send_time.map(|t| t.elapsed().as_secs_f64() * 1000.0);
                        results[idx].ip = Some(src_ip.to_string());
                        results[idx].rtt_ms = rtt_ms;
                    }
                    continue;
                }

                // 不是 ICMP → 尝试解析为来自目标的 TCP（SYN-ACK / RST）
                // dst_port 就是我们的源端口（每跳 BASE_SRC_PORT+ttl）→ 反查 ttl
                if let Some(ttl) = parse_target_tcp_for_ttl(packet.data, datalink, target, target_port) {
                    if ttl == 0 || ttl as usize > results.len() {
                        continue;
                    }
                    log::debug!(
                        "[pcap-tcp] target tcp recv: ttl={} (目标真正回复)",
                        ttl
                    );
                    update_target_hop(&mut found_target_hop, ttl, max_hops, target, &send_times, &mut results);
                }
            }
            Err(pcap::Error::TimeoutExpired) => {}
            Err(e) => {
                log::warn!("[pcap-tcp] next_packet 错误: {}", e);
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        // 注意：不再轮询 TCP socket connect 状态——Windows 下 socket 状态完全不可信
        // （ttl=1 的 socket 也会出现 peer_addr Ok 的假阳性）。
        // 真正可靠的目标判定 = pcap 抓到来自 target_ip:target_port 的 TCP 包。
    }

    drop(tcp_sockets);
    Ok(results)
}

/// 把"找到的目标 ttl"更新为更小值，并清掉污染、把目标行写为 target IP
fn update_target_hop(
    found_target_hop: &mut Option<u8>,
    ttl: u8,
    max_hops: u8,
    target: Ipv4Addr,
    send_times: &[Option<Instant>],
    results: &mut [FastHop],
) {
    let new_target_ttl = match *found_target_hop {
        Some(prev) => prev.min(ttl),
        None => ttl,
    };
    // 仅在最小目标跳确实更新时才打 INFO，避免重复探测时刷屏
    let updated = match *found_target_hop {
        Some(prev) => new_target_ttl < prev,
        None => true,
    };
    if updated {
        log::info!(
            "[pcap-tcp] update_target_hop ttl={} → new_target_ttl={}",
            ttl, new_target_ttl
        );
    } else {
        log::debug!(
            "[pcap-tcp] update_target_hop ttl={} → new_target_ttl={} (no change)",
            ttl, new_target_ttl
        );
    }
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
    *found_target_hop = Some(new_target_ttl);
}

/// 检查非阻塞 TCP connect 是否到达目标（成功 / ECONNREFUSED 都算到达）
#[allow(dead_code)]
fn check_tcp_connect_done(_sock: &Socket) -> Option<bool> {
    // 已废弃：Windows 下 TCP socket 状态完全不可信，所有 ttl 的 socket 都可能
    // 在 connect() 后立即让 peer_addr 返回 Ok 的假阳性。
    // 改用 pcap 抓"来自 target_ip:target_port 的 TCP 包"作为目标判定。
    None
}

/// 从 pcap 抓到的数据帧中解析"来自 target 的 TCP 包"，提取它对应的 ttl。
///
/// 流程：以太网 → IPv4(src=target) → TCP(src_port=target_port) → 看 dst_port，
/// dst_port 就是我们当时绑定的源端口（BASE_SRC_PORT + ttl），反算 ttl。
///
/// 返回 Some(ttl) 当且仅当：
/// - IPv4 src == target
/// - 协议 = TCP
/// - TCP src_port == target_port
/// - TCP dst_port - BASE_SRC_PORT 在 [1, 64] 范围内
fn parse_target_tcp_for_ttl(
    data: &[u8],
    datalink: pcap::Linktype,
    target: Ipv4Addr,
    target_port: u16,
) -> Option<u8> {
    let ip_data = strip_link_header(data, datalink)?;
    let ip = Ipv4Packet::new(ip_data)?;
    if ip.get_source() != target {
        return None;
    }
    if ip.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {
        return None;
    }
    let tcp = ip.payload();
    if tcp.len() < 4 {
        return None;
    }
    let src_port = u16::from_be_bytes([tcp[0], tcp[1]]);
    let dst_port = u16::from_be_bytes([tcp[2], tcp[3]]);
    if src_port != target_port {
        return None;
    }
    if dst_port < BASE_SRC_PORT {
        return None;
    }
    let ttl = dst_port - BASE_SRC_PORT;
    if ttl == 0 || ttl > 64 {
        return None;
    }
    Some(ttl as u8)
}

fn pick_default_device() -> AppResult<Device> {
    let devices = Device::list()
        .map_err(|e| AppError::TracerouteError(format!("pcap Device::list 失败: {}", e)))?;

    let local_ip = crate::utils::network::get_local_ip();
    log::info!("[pcap-tcp] 本机外发 IP: {:?}", local_ip);

    if let Some(std::net::IpAddr::V4(local_v4)) = local_ip {
        for d in devices.iter() {
            let matches = d.addresses.iter().any(|a| match a.addr {
                std::net::IpAddr::V4(v4) => v4 == local_v4,
                _ => false,
            });
            if matches {
                log::info!("[pcap-tcp] 通过本机 IP 匹配到网卡: {} ({})",
                    d.name, d.desc.as_deref().unwrap_or(""));
                return Ok(d.clone());
            }
        }
        log::warn!("[pcap-tcp] 没有 pcap 设备地址匹配本机 IP {}，退而求其次", local_v4);
    }

    let virtual_keywords = [
        "loopback", "Loopback", "LOOPBACK",
        "Hyper-V", "Virtual", "VMware", "VMnet",
        "WSL", "vEthernet", "Bluetooth", "Tunnel",
    ];
    for d in devices.iter() {
        let desc = d.desc.as_deref().unwrap_or("");
        if virtual_keywords.iter().any(|k| desc.contains(k)) {
            continue;
        }
        let has_ipv4 = d.addresses.iter().any(|a| match a.addr {
            std::net::IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_unspecified(),
            _ => false,
        });
        if has_ipv4 {
            log::info!("[pcap-tcp] 排除虚拟接口后选择: {} ({})", d.name, desc);
            return Ok(d.clone());
        }
    }

    for d in devices.iter() {
        let has_ipv4 = d.addresses.iter().any(|a| match a.addr {
            std::net::IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_unspecified(),
            _ => false,
        });
        if has_ipv4 {
            log::warn!("[pcap-tcp] 兜底选择: {} ({})", d.name, d.desc.as_deref().unwrap_or(""));
            return Ok(d.clone());
        }
    }

    Err(AppError::TracerouteError("未找到合适的网卡（无 IPv4 非 loopback 接口）".into()))
}

/// 从 pcap 抓到的数据帧中提取 (路由器 IP, TTL)。TTL 通过 ICMP body 内嵌 TCP 头的 src_port 反查。
fn parse_icmp_for_tcp(
    data: &[u8],
    datalink: pcap::Linktype,
) -> Option<(Ipv4Addr, u8)> {
    let ip_data = strip_link_header(data, datalink)?;
    let outer = Ipv4Packet::new(ip_data)?;
    if outer.get_next_level_protocol() != IpNextHeaderProtocols::Icmp {
        return None;
    }
    let outer_src = outer.get_source();

    let icmp = IcmpPacket::new(outer.payload())?;
    let icmp_type = icmp.get_icmp_type().0;
    if icmp_type != 11 && icmp_type != 3 {
        return None;
    }

    let icmp_payload = icmp.payload();
    if icmp_payload.len() < 4 + 20 + 4 {
        return None;
    }
    let inner_ip_buf = &icmp_payload[4..];
    let inner_ip = Ipv4Packet::new(inner_ip_buf)?;
    if inner_ip.get_next_level_protocol() != IpNextHeaderProtocols::Tcp {
        return None;
    }
    let inner_tcp = inner_ip.payload();
    if inner_tcp.len() < 4 {
        return None;
    }
    let src_port = u16::from_be_bytes([inner_tcp[0], inner_tcp[1]]);
    if src_port < BASE_SRC_PORT {
        return None;
    }
    let ttl = src_port - BASE_SRC_PORT;
    if ttl == 0 || ttl > 64 {
        return None;
    }
    Some((outer_src, ttl as u8))
}

fn strip_link_header<'a>(data: &'a [u8], datalink: pcap::Linktype) -> Option<&'a [u8]> {
    use pcap::Linktype;
    const ETH_HEADER_LEN: usize = 14;
    match datalink {
        Linktype::ETHERNET => {
            if data.len() < ETH_HEADER_LEN {
                return None;
            }
            let ethertype = u16::from_be_bytes([data[12], data[13]]);
            if ethertype != 0x0800 {
                return None;
            }
            Some(&data[ETH_HEADER_LEN..])
        }
        Linktype(12) | Linktype(228) => Some(data),
        _ => {
            if data.len() < ETH_HEADER_LEN {
                return None;
            }
            let ethertype = u16::from_be_bytes([data[12], data[13]]);
            if ethertype != 0x0800 {
                return None;
            }
            Some(&data[ETH_HEADER_LEN..])
        }
    }
}
