use std::net::IpAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::Duration;
use once_cell::sync::OnceCell;
use surge_ping::{Client, Config, ICMP, PingIdentifier, PingSequence};
use crate::error::{AppError, AppResult};

/// Global surge-ping clients (IPv4 / IPv6). `None` means the client failed to initialise
/// (likely due to insufficient privileges or platform restrictions) and we should fall
/// back to the system `ping` command.
static CLIENT_V4: OnceCell<Option<Arc<Client>>> = OnceCell::new();
static CLIENT_V6: OnceCell<Option<Arc<Client>>> = OnceCell::new();

/// 全局唯一 identifier 计数器。surge-ping 通过 (identifier, sequence) 路由 ICMP 回复给
/// 对应的 pinger,如果多个 pinger 同时使用相同 identifier+sequence 发包,就会触发
/// "Multiple identical request" 错误。
///
/// PingPlotter 多目标场景下(多个持续 ping 任务 + 持续 traceroute 每跳的 ping)很容易
/// 撞车,所以这里给每个 pinger 分配独立 identifier。16 位空间足够,溢出从 1 重新开始
/// (0 留作"未分配"标记,避免与初始化值混淆)。
static PING_ID_COUNTER: AtomicU16 = AtomicU16::new(1);

fn next_ping_identifier() -> PingIdentifier {
    let mut id = PING_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    if id == 0 {
        // 跳过 0,从 1 重新开始
        id = PING_ID_COUNTER.fetch_add(1, Ordering::Relaxed);
    }
    PingIdentifier(id)
}

/// Initialise raw-socket ICMP clients. Safe to call multiple times; only the first
/// call has any effect.
pub fn init() {
    CLIENT_V4.get_or_init(|| {
        let config = Config::builder().kind(ICMP::V4).build();
        match Client::new(&config) {
            Ok(c) => {
                log::info!("surge-ping IPv4 client initialised");
                Some(Arc::new(c))
            }
            Err(e) => {
                log::warn!("surge-ping IPv4 client init failed (will fall back to system ping): {}", e);
                None
            }
        }
    });

    CLIENT_V6.get_or_init(|| {
        let config = Config::builder().kind(ICMP::V6).build();
        match Client::new(&config) {
            Ok(c) => {
                log::info!("surge-ping IPv6 client initialised");
                Some(Arc::new(c))
            }
            Err(e) => {
                log::warn!("surge-ping IPv6 client init failed: {}", e);
                None
            }
        }
    });
}

/// Get the appropriate client for an IP family. Returns `None` if no client
/// is available (caller should fall back to system ping).
fn client_for(ip: IpAddr) -> Option<Arc<Client>> {
    match ip {
        IpAddr::V4(_) => CLIENT_V4.get().and_then(|o| o.clone()),
        IpAddr::V6(_) => CLIENT_V6.get().and_then(|o| o.clone()),
    }
}

/// Whether native (raw-socket) ICMP is available for the given IP.
pub fn is_native_available(ip: IpAddr) -> bool {
    client_for(ip).is_some()
}

/// Send one ICMP echo and wait for the reply.
///
/// Returns:
///   * `Ok(Some(ms))`  — round-trip time in milliseconds
///   * `Ok(None)`      — request timed out
///   * `Err(...)`      — surge-ping error; caller may decide to fall back
pub async fn ping_native(
    ip: IpAddr,
    seq: u16,
    timeout_ms: u32,
    packet_size: u32,
) -> AppResult<Option<f64>> {
    let client = client_for(ip)
        .ok_or_else(|| AppError::PingError("native ICMP client unavailable".into()))?;

    // 每次发 ping 都分配一个唯一 identifier,避免 surge-ping 在多 pinger 并发时
    // 因 (identifier, sequence) 撞车报 "Multiple identical request"。
    let ident = next_ping_identifier();
    let mut pinger = client.pinger(ip, ident).await;
    pinger.timeout(Duration::from_millis(timeout_ms as u64));

    // surge-ping pads its own 8-byte ICMP header; ensure we send at least 16 bytes of payload
    // to keep behaviour consistent with the system `ping -l` flag.
    let payload_len = packet_size.max(16) as usize;
    let payload = vec![0x61u8; payload_len];

    match pinger.ping(PingSequence(seq), &payload).await {
        Ok((_packet, duration)) => {
            let ms = duration.as_secs_f64() * 1000.0;
            Ok(Some(ms))
        }
        Err(surge_ping::SurgeError::Timeout { .. }) => Ok(None),
        Err(e) => Err(AppError::PingError(format!("surge-ping error: {}", e))),
    }
}
