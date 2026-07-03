// 通过 Npcap 实现真正的并行 UDP traceroute（仅 Windows）+ 独立 recv 线程
//
// 与 services::fast_udp_traceroute 的核心差别：
// - fast_udp_traceroute 用标准 UdpSocket 发包 + raw ICMP socket 收包
//   → Windows 内核会过滤掉由本机 UDP 触发的 ICMP Time Exceeded → 中间跳收不到
// - 本实现用 pcap（Npcap）直接在网卡层抓所有 ICMP，绕过 OS 过滤
//   → 真正能拿到中间跳路由器的 IP
//
// 关键：send / recv 分离（与 fast_udp_traceroute 相同结构）
// - 独立 recv 线程：pcap.next_packet 循环，收到 ICMP 包立即 Instant::now() 打时间戳,
//   解析后通过 mpsc 发给主线程 → 首跳 RTT 不再被 send 阶段的耗时污染
// - Windows 上"每 TTL 一个 UdpSocket bind"耗时 1000ms+,若同步阻塞收包会导致
//   首跳 RTT 虚高 ~1000ms（bug 现场,与 fast_udp 完全一致）
// - 保留了每 TTL 独立 src_port 的语义（能观察到 UDP ECMP 分岔）

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pcap::{Capture, Device};
use pnet_packet::Packet;
use pnet_packet::ipv4::Ipv4Packet;
use pnet_packet::ip::IpNextHeaderProtocols;
use pnet_packet::icmp::IcmpPacket;

use crate::error::{AppError, AppResult};
use crate::services::fast_traceroute::FastHop;

/// 标准 traceroute 起始端口
const BASE_PORT: u16 = 33434;

/// 一条 ICMP 响应事件：由 recv 线程实时打时间戳,通过 mpsc 发给主线程处理
struct RecvEvent {
    recv_time: Instant,
    src_ip: Ipv4Addr,
    ttl: u8,
}

/// 通过 Npcap 实现的并行 UDP traceroute。target 必须是已解析的 IPv4。
///
/// 调用方应先确认 Npcap 已安装（detect_npcap().installed == true），否则会失败。
pub async fn parallel_udp_traceroute(
    target: Ipv4Addr,
    max_hops: u32,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let max_hops = max_hops.clamp(1, 64) as u8;

    // pcap 操作是阻塞 IO，丢到 spawn_blocking
    let result = tokio::task::spawn_blocking(move || -> AppResult<Vec<FastHop>> {
        do_pcap_udp_traceroute(target, max_hops, timeout_ms)
    })
    .await
    .map_err(|e| AppError::TracerouteError(format!("pcap UDP traceroute task join 失败: {}", e)))??;

    Ok(result)
}

fn do_pcap_udp_traceroute(
    target: Ipv4Addr,
    max_hops: u8,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    // 1) 选默认外发网卡
    let device = pick_default_device()?;
    log::info!(
        "[pcap-udp] 使用网卡 {} ({})",
        device.name,
        device.desc.as_deref().unwrap_or("无描述")
    );

    // 2) 打开 capture 句柄
    //    - immediate_mode：立刻把包送上来（不等内核缓冲填满）
    //    - timeout：read 阻塞最长时间（毫秒）；用 50ms 让 recv 线程能定期检查 stop_flag
    //    - snaplen：1500 足够包住完整以太网帧 + 嵌入 IP + 嵌入 UDP 头
    let mut cap = Capture::from_device(device.clone())
        .map_err(|e| AppError::TracerouteError(format!("pcap from_device 失败: {}", e)))?
        .promisc(false)
        .immediate_mode(true)
        .timeout(50)
        .snaplen(1500)
        .open()
        .map_err(|e| AppError::TracerouteError(format!("pcap open 失败（Npcap 是否已启动？）: {}", e)))?;

    // BPF 过滤：只关心 ICMP 包（中间跳的 Time Exceeded、目标的 Port Unreachable）
    if let Err(e) = cap.filter("icmp", true) {
        log::warn!("[pcap-udp] 设置 BPF 过滤失败（{}），将抓全部包并自行过滤", e);
    }

    let datalink = cap.get_datalink();
    log::info!("[pcap-udp] 链路类型: {:?}", datalink);

    let stop_flag = Arc::new(AtomicBool::new(false));
    let stop_flag_recv = stop_flag.clone();
    let (tx, rx) = mpsc::channel::<RecvEvent>();

    // ===== 独立 recv 线程 =====
    // 一开始就在 cap.next_packet 上等 ICMP。收到即用 Instant::now() 打时间戳,
    // 解析（跳过链路头 + IP + ICMP + 内嵌 IP + 内嵌 UDP）后反查 TTL,通过 mpsc
    // 发给主线程。目的：send 阶段的耗时不再污染 RTT 计算。
    let recv_thread = std::thread::spawn(move || {
        while !stop_flag_recv.load(Ordering::Relaxed) {
            match cap.next_packet() {
                Ok(packet) => {
                    let recv_time = Instant::now();
                    if let Some((src_ip, ttl)) = parse_icmp_for_udp(packet.data, datalink) {
                        if tx.send(RecvEvent {
                            recv_time,
                            src_ip,
                            ttl,
                        }).is_err() {
                            // 主线程 rx 已关闭
                            break;
                        }
                    }
                }
                Err(pcap::Error::TimeoutExpired) => {
                    // pcap 50ms 内没收到包,正常情况
                    continue;
                }
                Err(e) => {
                    log::warn!("[pcap-udp] recv thread error: {}", e);
                    std::thread::sleep(Duration::from_millis(50));
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

    // ===== Send 阶段：每 TTL 独立 UdpSocket =====
    // 用标准 UdpSocket 发包（让 OS 处理 ARP/路由），pcap 那边抓回程 ICMP。
    // 每 TTL 独立 socket 保留 UDP ECMP 观察能力（OS 每次分配不同 src_port）。
    let mut udp_sockets: Vec<UdpSocket> = Vec::with_capacity(max_hops as usize);
    for ttl in 1..=max_hops {
        let sock = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                log::warn!("[pcap-udp] UDP bind 失败 ttl={}: {}", ttl, e);
                continue;
            }
        };
        if let Err(e) = sock.set_ttl(ttl as u32) {
            log::warn!("[pcap-udp] UDP set_ttl({}) 失败: {}", ttl, e);
            continue;
        }

        let dst_port = BASE_PORT + ttl as u16;
        let dst: SocketAddr = SocketAddrV4::new(target, dst_port).into();

        send_times[ttl as usize] = Some(Instant::now());
        if let Err(e) = sock.send_to(&[0u8; 1], dst) {
            log::warn!("[pcap-udp] send_to ttl={} 失败: {}", ttl, e);
        }
        udp_sockets.push(sock);

        // 顺手消化 channel 里已到达的响应
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
    stop_flag.store(true, Ordering::Relaxed);
    let _ = recv_thread.join();

    drop(udp_sockets);
    Ok(results)
}

/// 处理一条 recv 线程发来的 ICMP 事件。
/// RTT = evt.recv_time - send_times[ttl]，反映真实往返时间（不含 send 阶段耗时）。
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
        "[pcap-udp] icmp recv: src={} reverse_ttl={} from_target={}",
        evt.src_ip, ttl, from_target
    );

    if from_target {
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

    let idx = (ttl - 1) as usize;
    if results[idx].ip.is_some() {
        return;
    }
    let send_time = send_times[ttl as usize];
    let rtt_ms = send_time.map(|t| (evt.recv_time - t).as_secs_f64() * 1000.0);
    results[idx].ip = Some(evt.src_ip.to_string());
    results[idx].rtt_ms = rtt_ms;
}

/// 选择默认发包用的网卡：必须匹配本机外发流量的真实出口 IP，
/// 否则可能选到 Hyper-V/VMware/WSL2 等虚拟网卡（没有真实流量）。
fn pick_default_device() -> AppResult<Device> {
    let devices = Device::list()
        .map_err(|e| AppError::TracerouteError(format!("pcap Device::list 失败: {}", e)))?;

    // 1) 拿到本机真实外发 IP（通过 connect 8.8.8.8 让 OS 选默认路由）
    let local_ip = crate::utils::network::get_local_ip();
    log::info!("[pcap-udp] 本机外发 IP: {:?}", local_ip);

    // 2) 优先：匹配该 IP 的 pcap 设备
    if let Some(std::net::IpAddr::V4(local_v4)) = local_ip {
        for d in devices.iter() {
            let matches = d.addresses.iter().any(|a| match a.addr {
                std::net::IpAddr::V4(v4) => v4 == local_v4,
                _ => false,
            });
            if matches {
                log::info!("[pcap-udp] 通过本机 IP 匹配到网卡: {} ({})",
                    d.name, d.desc.as_deref().unwrap_or(""));
                return Ok(d.clone());
            }
        }
        log::warn!("[pcap-udp] 没有 pcap 设备地址匹配本机 IP {}，退而求其次", local_v4);
    }

    // 3) 退而求其次：跳过明显的虚拟网卡，选第一个非虚拟、有 IPv4 地址的
    let virtual_keywords = [
        "loopback", "Loopback", "LOOPBACK",
        "Hyper-V", "Virtual", "VMware", "VMnet",
        "WSL", "vEthernet", "Bluetooth", "Tunnel",
    ];
    for d in devices.iter() {
        let desc = d.desc.as_deref().unwrap_or("");
        let is_virtual = virtual_keywords.iter().any(|k| desc.contains(k));
        if is_virtual {
            continue;
        }
        let has_ipv4 = d.addresses.iter().any(|a| match a.addr {
            std::net::IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_unspecified(),
            _ => false,
        });
        if has_ipv4 {
            log::info!("[pcap-udp] 排除虚拟接口后选择: {} ({})", d.name, desc);
            return Ok(d.clone());
        }
    }

    // 4) 最后兜底：列表里第一个有 IPv4 的
    for d in devices.iter() {
        let has_ipv4 = d.addresses.iter().any(|a| match a.addr {
            std::net::IpAddr::V4(v4) => !v4.is_loopback() && !v4.is_unspecified(),
            _ => false,
        });
        if has_ipv4 {
            log::warn!("[pcap-udp] 兜底选择: {} ({})", d.name, d.desc.as_deref().unwrap_or(""));
            return Ok(d.clone());
        }
    }

    Err(AppError::TracerouteError(
        "未找到合适的网卡（无 IPv4 非 loopback 接口）".into(),
    ))
}

/// 从 pcap 抓到的原始数据帧中提取 (中间路由器 IP, 原始 UDP 包对应的 TTL)。
///
/// 数据帧布局（以太网）：
///   [Ethernet 14B] [IPv4 20+B] [ICMP body...]
/// ICMP body（type=11 Time Exceeded 或 type=3 Dest Unreachable）：
///   [ICMP 头 4B] [unused 4B] [原始 IPv4 头 20+B] [原始 UDP 头 8B]
/// 原始 UDP 头里 dst_port 的低位 = TTL（dst_port - 33434 = ttl）
fn parse_icmp_for_udp(
    data: &[u8],
    datalink: pcap::Linktype,
) -> Option<(Ipv4Addr, u8)> {
    // 跳过链路层头部
    let ip_data = strip_link_header(data, datalink)?;

    // 解析外层 IPv4
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

    // ICMP body: 4B header + 4B unused + 内嵌 IPv4 header + 内嵌 UDP 头
    let icmp_payload = icmp.payload();
    if icmp_payload.len() < 4 + 20 + 8 {
        return None;
    }
    // 跳过前 4B unused（payload 不包含 ICMP header 自己的 type/code/checksum，
    // pnet 把它们解出去了；但根据 RFC 792，type=11/3 的 ICMP 内还有 4B unused 在 body 开头）
    let inner_ip_buf = &icmp_payload[4..];
    let inner_ip = Ipv4Packet::new(inner_ip_buf)?;
    if inner_ip.get_next_level_protocol() != IpNextHeaderProtocols::Udp {
        return None;
    }
    let inner_payload = inner_ip.payload();
    if inner_payload.len() < 4 {
        return None;
    }
    // UDP 头前 4B: src_port(2) + dst_port(2)
    let dst_port = u16::from_be_bytes([inner_payload[2], inner_payload[3]]);
    if dst_port < BASE_PORT {
        return None;
    }
    let ttl = (dst_port - BASE_PORT) as u8;
    if ttl == 0 || ttl > 64 {
        return None;
    }

    Some((outer_src, ttl))
}

/// 跳过链路层头部，返回 IP 层起始的 slice。
/// 目前支持 Ethernet（最常见）；其它链路类型直接返回原数据。
fn strip_link_header<'a>(data: &'a [u8], datalink: pcap::Linktype) -> Option<&'a [u8]> {
    use pcap::Linktype;
    // 以太网头固定 14 字节：dst(6) + src(6) + ethertype(2)
    const ETH_HEADER_LEN: usize = 14;
    match datalink {
        Linktype::ETHERNET => {
            if data.len() < ETH_HEADER_LEN {
                return None;
            }
            // ethertype 在 offset 12..14
            let ethertype = u16::from_be_bytes([data[12], data[13]]);
            // 0x0800 = IPv4
            if ethertype != 0x0800 {
                return None;
            }
            Some(&data[ETH_HEADER_LEN..])
        }
        // RAW/IPV4 链路类型：数据直接从 IP 头开始
        Linktype(12) /* DLT_RAW */ | Linktype(228) /* DLT_IPV4 */ => Some(data),
        _ => {
            // 其它链路类型先按 Ethernet 试
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
