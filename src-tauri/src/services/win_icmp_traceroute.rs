// Windows 平台 ICMP 并行 traceroute（基于 IcmpSendEcho2 API）
//
// 为什么独立成一个模块：
//   Windows 用户态 raw ICMP socket 即使管理员权限，内核也不会可靠地把
//   中间路由器发来的 ICMP Time Exceeded 上送过来，导致 fast_traceroute 的
//   raw socket 实现只能拿到目标主机的 Echo Reply（一跳）。
//   官方 tracert.exe / WinMTR 都不走 raw socket，而是调用 iphlpapi.dll 的
//   IcmpSendEcho / IcmpSendEcho2，由内核 ICMP 引擎处理 TTL + 收响应，
//   把"中间跳的 Time Exceeded"作为一条 ECHO_REPLY 记录返回给应用，
//   status 字段标记为 IP_TTL_EXPIRED_TRANSIT(11013)。
//
// 并行策略：
//   每个 TTL 用一个 handle + 独立 OS 线程（tokio::spawn_blocking）调用
//   阻塞版 IcmpSendEcho2，整体耗时 ≈ 单个 timeout 窗口。
//   也可以用异步版（带 Event 句柄 + WaitForMultipleObjects），但代码复杂度
//   高一个量级，对 30 跳的小规模并发没必要。

#![cfg(windows)]

use std::ffi::c_void;
use std::net::Ipv4Addr;
use std::ptr;
use std::time::Instant;

use crate::error::{AppError, AppResult};
use crate::services::fast_traceroute::FastHop;

// -- FFI 绑定（iphlpapi.dll） ------------------------------------------------

#[link(name = "iphlpapi")]
extern "system" {
    fn IcmpCreateFile() -> *mut c_void;
    fn IcmpCloseHandle(handle: *mut c_void) -> i32;
    fn IcmpSendEcho2(
        handle: *mut c_void,
        event: *mut c_void,
        apc_routine: *mut c_void,
        apc_context: *mut c_void,
        destination_address: u32, // IPAddr：在 LE 机器上字节序为 [a,b,c,d]→ a.b.c.d
        request_data: *const c_void,
        request_size: u16,
        request_options: *const IpOptionInformation,
        reply_buffer: *mut c_void,
        reply_size: u32,
        timeout: u32,
    ) -> u32; // 返回回复个数；0 表示无回复（超时或错误）
}

// IP_OPTION_INFORMATION：用于设置 TTL 等
#[repr(C)]
struct IpOptionInformation {
    ttl: u8,
    tos: u8,
    flags: u8,
    options_size: u8,
    options_data: *mut u8,
}

// ICMP_ECHO_REPLY：IcmpSendEcho2 返回的响应结构
//
// 注意：在 64 位 Windows 上 data 指针是 8 字节，导致整个 struct 因为
// 自然对齐变成 32 字节（不是 28）。Rust 的 #[repr(C)] 自动处理这件事，
// 这里写下大小只是给 reply_buffer 计算时参考。
#[repr(C)]
struct IcmpEchoReply {
    address: u32,         // 回复方 IP（IPAddr/ULONG）
    status: u32,          // IP_STATUS，见下面常量
    round_trip_time: u32, // 毫秒
    data_size: u16,
    reserved: u16,
    data: *mut c_void,
    options: IpOptionInformation,
}

// 常用 IP_STATUS 值（见 ipexport.h）
const IP_SUCCESS: u32 = 0;
const IP_TTL_EXPIRED_TRANSIT: u32 = 11013;
const IP_TTL_EXPIRED_REASSEM: u32 = 11014;
// 其他失败码（IP_DEST_HOST_UNREACHABLE 等）一律视为该跳无效。

// -- 公开 API ---------------------------------------------------------------

/// 在 Windows 上做并行 ICMP traceroute。
/// target：已解析的 IPv4 地址。
/// max_hops：最大跳数（1..=64 之间）。
/// timeout_ms：单跳超时（毫秒）。
pub async fn parallel_icmp_traceroute(
    target: Ipv4Addr,
    max_hops: u32,
    timeout_ms: u32,
) -> AppResult<Vec<FastHop>> {
    let max_hops = max_hops.clamp(1, 64) as u8;

    // 一次性 spawn N 个阻塞任务，让 OS 线程池并发跑。tokio 默认 blocking 池
    // 容量 512，足够 30~64 跳并发。
    let mut handles = Vec::with_capacity(max_hops as usize);
    for ttl in 1..=max_hops {
        let target = target;
        let h = tokio::task::spawn_blocking(move || send_one_hop(target, ttl, timeout_ms));
        handles.push((ttl, h));
    }

    // 收集结果：保持 ttl 顺序填入 results
    let mut results: Vec<FastHop> = (1..=max_hops as u32)
        .map(|n| FastHop {
            hop_number: n,
            ip: None,
            rtt_ms: None,
        })
        .collect();
    let mut found_target_ttl: Option<u8> = None;
    let target_str = target.to_string();

    for (ttl, h) in handles {
        let idx = (ttl - 1) as usize;
        match h.await {
            Ok(Some(hop)) => {
                if let Some(ref ip) = hop.ip {
                    if *ip == target_str {
                        // 记录目标首次出现的 ttl，用于后面截断
                        found_target_ttl = Some(match found_target_ttl {
                            Some(prev) => prev.min(ttl),
                            None => ttl,
                        });
                    }
                }
                results[idx].ip = hop.ip;
                results[idx].rtt_ms = hop.rtt_ms;
            }
            Ok(None) => {
                // 超时：保持 ip=None
            }
            Err(e) => {
                log::debug!("[win_icmp_traceroute] ttl={} task join 失败: {}", ttl, e);
            }
        }
    }

    // 截断到第一次到达目标的跳，避免后面一长串空跳
    if let Some(target_ttl) = found_target_ttl {
        results.truncate(target_ttl as usize);
    }

    // 检查至少有一个有效跳；否则提示调用方走兜底
    let any_resolved = results.iter().any(|h| h.ip.is_some());
    if !any_resolved {
        return Err(AppError::TracerouteError(
            "IcmpSendEcho2 未拿到任何跳响应（可能 iphlpapi 不可用或防火墙阻断）".into(),
        ));
    }

    Ok(results)
}

// -- 内部实现 ---------------------------------------------------------------

/// 单跳同步调用，返回该跳结果。None 表示超时或失败（reply count = 0）。
fn send_one_hop(target: Ipv4Addr, ttl: u8, timeout_ms: u32) -> Option<FastHop> {
    // 用 catch_unwind 包一层，防止 unsafe FFI 极端情况下 panic 炸进 tokio 线程池
    let r = std::panic::catch_unwind(|| send_one_hop_inner(target, ttl, timeout_ms));
    match r {
        Ok(v) => v,
        Err(_) => {
            log::warn!("[win_icmp_traceroute] ttl={} FFI 调用 panic", ttl);
            None
        }
    }
}

fn send_one_hop_inner(target: Ipv4Addr, ttl: u8, timeout_ms: u32) -> Option<FastHop> {
    unsafe {
        let handle = IcmpCreateFile();
        if handle.is_null() || handle as isize == -1 {
            log::warn!("[win_icmp_traceroute] IcmpCreateFile 失败，ttl={}", ttl);
            return None;
        }

        // RAII 守卫，确保 handle 一定关闭
        struct HandleGuard(*mut c_void);
        impl Drop for HandleGuard {
            fn drop(&mut self) {
                unsafe {
                    IcmpCloseHandle(self.0);
                }
            }
        }
        let _guard = HandleGuard(handle);

        let opts = IpOptionInformation {
            ttl,
            tos: 0,
            flags: 0,
            options_size: 0,
            options_data: ptr::null_mut(),
        };

        // 32 字节 payload，与 system ping 默认大小一致
        let request_data: [u8; 32] = [0x61; 32];

        // reply 缓冲：ICMP_ECHO_REPLY 自身 + 回包 payload + 8 字节 IO_STATUS_BLOCK
        // MSDN 推荐至少 sizeof(ICMP_ECHO_REPLY) + RequestSize + 8
        let reply_buf_size = std::mem::size_of::<IcmpEchoReply>() + request_data.len() + 8;
        let mut reply_buf = vec![0u8; reply_buf_size];

        // IPAddr DWORD：把 [a,b,c,d] 按小端组成 u32（Windows x86/x64 均为 LE）
        let octets = target.octets();
        let dest = u32::from_le_bytes(octets);

        let send_at = Instant::now();
        let count = IcmpSendEcho2(
            handle,
            ptr::null_mut(), // 同步调用
            ptr::null_mut(),
            ptr::null_mut(),
            dest,
            request_data.as_ptr() as *const c_void,
            request_data.len() as u16,
            &opts,
            reply_buf.as_mut_ptr() as *mut c_void,
            reply_buf_size as u32,
            timeout_ms,
        );

        if count == 0 {
            // 超时或错误，无回复
            return None;
        }

        // 读第一条回复
        let reply = &*(reply_buf.as_ptr() as *const IcmpEchoReply);

        // 只接受 IP_SUCCESS（目标）和 IP_TTL_EXPIRED_*（中间跳）。
        // 其他状态（destination unreachable 等）说明这一跳数据不可用。
        match reply.status {
            IP_SUCCESS | IP_TTL_EXPIRED_TRANSIT | IP_TTL_EXPIRED_REASSEM => {
                let bytes = reply.address.to_le_bytes();
                let ip = Ipv4Addr::new(bytes[0], bytes[1], bytes[2], bytes[3]);

                // round_trip_time 有时候为 0（小于 1ms）。用我们自己的计时更准。
                let rtt_self = send_at.elapsed().as_secs_f64() * 1000.0;
                let rtt_ms = if reply.round_trip_time > 0 {
                    reply.round_trip_time as f64
                } else {
                    rtt_self.max(0.1)
                };

                Some(FastHop {
                    hop_number: ttl as u32,
                    ip: Some(ip.to_string()),
                    rtt_ms: Some(rtt_ms),
                })
            }
            other => {
                log::debug!(
                    "[win_icmp_traceroute] ttl={} status=0x{:X} 视为该跳无效",
                    ttl,
                    other
                );
                None
            }
        }
    }
}
