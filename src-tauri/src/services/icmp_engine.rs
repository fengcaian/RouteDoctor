use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;
use once_cell::sync::OnceCell;
use surge_ping::{Client, Config, ICMP, PingIdentifier, PingSequence};
use crate::error::{AppError, AppResult};

/// Global surge-ping clients (IPv4 / IPv6). `None` means the client failed to initialise
/// (likely due to insufficient privileges or platform restrictions) and we should fall
/// back to the system `ping` command.
static CLIENT_V4: OnceCell<Option<Arc<Client>>> = OnceCell::new();
static CLIENT_V6: OnceCell<Option<Arc<Client>>> = OnceCell::new();

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

    // Identifier should be unique per pinger; use process id + seq for low collision risk.
    let ident = PingIdentifier((std::process::id() & 0xFFFF) as u16);
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
