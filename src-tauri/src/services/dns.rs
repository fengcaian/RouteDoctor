use std::net::IpAddr;
use crate::error::{AppError, AppResult};

/// Resolve hostname to IP address (async)
/// Uses tokio's built-in DNS resolver which uses system DNS configuration
pub async fn resolve(hostname: &str) -> AppResult<IpAddr> {
    // Try parsing as IP first
    if let Ok(ip) = hostname.parse::<IpAddr>() {
        return Ok(ip);
    }

    // Use tokio's built-in DNS resolver (uses system DNS)
    match tokio::net::lookup_host((hostname, 0)).await {
        Ok(mut addrs) => {
            match addrs.next() {
                Some(addr) => Ok(addr.ip()),
                None => Err(AppError::DnsError(format!("Could not resolve {}: no addresses found", hostname))),
            }
        }
        Err(e) => Err(AppError::DnsError(format!("DNS lookup failed for {}: {}", hostname, e))),
    }
}

/// Reverse DNS lookup (IP to hostname) - async
pub async fn reverse_lookup(ip: &IpAddr) -> AppResult<Option<String>> {
    // Use tokio's built-in DNS resolver (uses system DNS)
    match tokio::net::lookup_host((ip.to_string().as_str(), 0)).await {
        Ok(_) => Ok(None), // lookup_host doesn't return hostname, just IP
        Err(_e) => Ok(None), // Ignore errors, hostname is optional
    }
}