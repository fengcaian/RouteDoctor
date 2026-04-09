use std::net::IpAddr;
use trust_dns_resolver::TokioAsyncResolver;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};
use crate::error::{AppError, AppResult};

/// Resolve hostname to IP address (async)
pub async fn resolve(hostname: &str) -> AppResult<IpAddr> {
    // Try parsing as IP first
    if let Ok(ip) = hostname.parse::<IpAddr>() {
        return Ok(ip);
    }

    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
    let response = resolver.lookup_ip(hostname)
        .await
        .map_err(|e| AppError::DnsError(e.to_string()))?;

    match response.iter().next() {
        Some(ip) => Ok(ip),
        None => Err(AppError::DnsError(format!("Could not resolve {}", hostname))),
    }
}

/// Reverse DNS lookup (IP to hostname) - async
pub async fn reverse_lookup(ip: &IpAddr) -> AppResult<Option<String>> {
    let resolver = TokioAsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());

    let response = resolver.reverse_lookup(*ip)
        .await
        .map_err(|e| AppError::DnsError(e.to_string()))?;

    match response.iter().next() {
        Some(name) => Ok(Some(name.to_string())),
        None => Ok(None),
    }
}