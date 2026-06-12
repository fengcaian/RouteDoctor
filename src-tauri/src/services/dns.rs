use std::net::IpAddr;
use std::time::Duration;
use once_cell::sync::Lazy;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::*;
use crate::error::{AppError, AppResult};

/// 反向 DNS 单次查询硬超时。trust-dns-resolver 默认会重试导致整体可能超过 10s,
/// 这里再叠加一层 tokio::time::timeout 保证 traceroute 富化阶段不被拖慢。
const REVERSE_LOOKUP_TIMEOUT: Duration = Duration::from_secs(2);

/// Global async DNS resolver. Falls back to public DNS providers if the
/// system configuration is unavailable.
pub static RESOLVER: Lazy<TokioAsyncResolver> = Lazy::new(|| {
    match TokioAsyncResolver::tokio_from_system_conf() {
        Ok(r) => r,
        Err(e) => {
            log::warn!("Failed to load system DNS config: {}, falling back to public DNS", e);
            let ips: Vec<IpAddr> = vec![
                "1.1.1.1".parse().unwrap(),   // Cloudflare
                "8.8.8.8".parse().unwrap(),   // Google
                "223.5.5.5".parse().unwrap(), // AliDNS (China-friendly)
            ];
            let config = ResolverConfig::from_parts(
                None,
                vec![],
                NameServerConfigGroup::from_ips_clear(&ips, 53, true),
            );
            TokioAsyncResolver::tokio(config, ResolverOpts::default())
        }
    }
});

/// Resolve hostname to IP address (async).
pub async fn resolve(hostname: &str) -> AppResult<IpAddr> {
    if let Ok(ip) = hostname.parse::<IpAddr>() {
        return Ok(ip);
    }

    let response = RESOLVER
        .lookup_ip(hostname)
        .await
        .map_err(|e| AppError::DnsError(format!("DNS lookup failed for {}: {}", hostname, e)))?;

    response
        .iter()
        .next()
        .ok_or_else(|| AppError::DnsError(format!("Could not resolve {}: no addresses found", hostname)))
}

/// Reverse DNS lookup (IP -> hostname). Failure is not treated as an error;
/// the function returns `Ok(None)` for any lookup miss.
/// 内部硬超时 2s,避免单条记录把 traceroute 富化阶段拖慢。
pub async fn reverse_lookup(ip: &IpAddr) -> AppResult<Option<String>> {
    let fut = RESOLVER.reverse_lookup(*ip);
    match tokio::time::timeout(REVERSE_LOOKUP_TIMEOUT, fut).await {
        Ok(Ok(response)) => {
            if let Some(name) = response.iter().next() {
                let mut s = name.to_string();
                if s.ends_with('.') {
                    s.pop();
                }
                if s.is_empty() { Ok(None) } else { Ok(Some(s)) }
            } else {
                Ok(None)
            }
        }
        // 解析失败或超时都视为"没有 hostname",不影响主流程
        Ok(Err(_)) => Ok(None),
        Err(_) => {
            log::debug!("reverse_lookup({}) timed out after {:?}", ip, REVERSE_LOOKUP_TIMEOUT);
            Ok(None)
        }
    }
}
