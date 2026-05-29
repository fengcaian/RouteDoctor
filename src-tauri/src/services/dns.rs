use std::net::IpAddr;
use once_cell::sync::Lazy;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::*;
use crate::error::{AppError, AppResult};

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
pub async fn reverse_lookup(ip: &IpAddr) -> AppResult<Option<String>> {
    match RESOLVER.reverse_lookup(*ip).await {
        Ok(response) => {
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
        Err(_) => Ok(None),
    }
}
