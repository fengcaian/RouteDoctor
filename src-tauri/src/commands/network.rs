use crate::error::{AppError, AppResult};
use serde::Serialize;

/// DNS 查询结果
#[derive(Debug, Serialize)]
pub struct DnsRecord {
    pub record_type: String,
    pub value: String,
    pub ttl: u32,
}

/// DNS 查询结果
#[derive(Debug, Serialize)]
pub struct DnsQueryResult {
    pub domain: String,
    pub records: Vec<DnsRecord>,
    pub query_time_ms: f64,
}

/// 网络接口信息
#[derive(Debug, Serialize)]
pub struct NetworkInterface {
    pub name: String,
    pub ip: String,
    pub interface_type: String,
}

/// 网络信息
#[derive(Debug, Serialize)]
pub struct NetworkInfo {
    pub local_ip: Option<String>,
    pub interfaces: Vec<NetworkInterface>,
    pub default_gateway: Option<String>,
    pub dns_servers: Vec<String>,
    pub hostname: String,
}

/// DNS 查询命令
#[tauri::command]
pub async fn dns_lookup(domain: String, record_type: String) -> AppResult<DnsQueryResult> {
    use trust_dns_resolver::TokioAsyncResolver;
    use trust_dns_resolver::config::*;
    use std::time::Instant;

    let resolver = TokioAsyncResolver::tokio(
        ResolverConfig::default(),
        ResolverOpts::default(),
    );

    let start = Instant::now();
    let mut records = Vec::new();

    match record_type.as_str() {
        "A" => {
            match resolver.lookup_ip(&domain).await {
                Ok(response) => {
                    for ip in response.iter() {
                        if ip.is_ipv4() {
                            records.push(DnsRecord {
                                record_type: "A".to_string(),
                                value: ip.to_string(),
                                ttl: response.as_lookup().record_iter().next()
                                    .map(|r| r.ttl()).unwrap_or(0),
                            });
                        }
                    }
                }
                Err(e) => return Err(AppError::DnsError(format!("A 记录查询失败: {}", e))),
            }
        }
        "AAAA" => {
            match resolver.lookup_ip(&domain).await {
                Ok(response) => {
                    for ip in response.iter() {
                        if ip.is_ipv6() {
                            records.push(DnsRecord {
                                record_type: "AAAA".to_string(),
                                value: ip.to_string(),
                                ttl: response.as_lookup().record_iter().next()
                                    .map(|r| r.ttl()).unwrap_or(0),
                            });
                        }
                    }
                }
                Err(e) => return Err(AppError::DnsError(format!("AAAA 记录查询失败: {}", e))),
            }
        }
        "CNAME" => {
            match resolver.lookup(
                &domain,
                trust_dns_resolver::proto::rr::RecordType::CNAME,
            ).await {
                Ok(response) => {
                    for record in response.record_iter() {
                        if let Some(data) = record.data() {
                            records.push(DnsRecord {
                                record_type: "CNAME".to_string(),
                                value: data.to_string(),
                                ttl: record.ttl(),
                            });
                        }
                    }
                }
                Err(_) => {} // CNAME 可能不存在，不报错
            }
        }
        "MX" => {
            match resolver.mx_lookup(&domain).await {
                Ok(response) => {
                    for mx in response.iter() {
                        records.push(DnsRecord {
                            record_type: "MX".to_string(),
                            value: format!("{} (优先级: {})", mx.exchange(), mx.preference()),
                            ttl: response.as_lookup().record_iter().next()
                                .map(|r| r.ttl()).unwrap_or(0),
                        });
                    }
                }
                Err(_) => {} // MX 可能不存在
            }
        }
        "NS" => {
            match resolver.ns_lookup(&domain).await {
                Ok(response) => {
                    for ns in response.iter() {
                        records.push(DnsRecord {
                            record_type: "NS".to_string(),
                            value: ns.to_string(),
                            ttl: response.as_lookup().record_iter().next()
                                .map(|r| r.ttl()).unwrap_or(0),
                        });
                    }
                }
                Err(_) => {}
            }
        }
        "TXT" => {
            match resolver.txt_lookup(&domain).await {
                Ok(response) => {
                    for txt in response.iter() {
                        records.push(DnsRecord {
                            record_type: "TXT".to_string(),
                            value: txt.to_string(),
                            ttl: response.as_lookup().record_iter().next()
                                .map(|r| r.ttl()).unwrap_or(0),
                        });
                    }
                }
                Err(_) => {}
            }
        }
        "ALL" => {
            // 查询所有常见记录类型
            // A 记录
            if let Ok(response) = resolver.lookup_ip(&domain).await {
                for ip in response.iter() {
                    records.push(DnsRecord {
                        record_type: if ip.is_ipv4() { "A" } else { "AAAA" }.to_string(),
                        value: ip.to_string(),
                        ttl: response.as_lookup().record_iter().next()
                            .map(|r| r.ttl()).unwrap_or(0),
                    });
                }
            }
            // CNAME
            if let Ok(response) = resolver.lookup(
                &domain,
                trust_dns_resolver::proto::rr::RecordType::CNAME,
            ).await {
                for record in response.record_iter() {
                    if let Some(data) = record.data() {
                        records.push(DnsRecord {
                            record_type: "CNAME".to_string(),
                            value: data.to_string(),
                            ttl: record.ttl(),
                        });
                    }
                }
            }
            // MX
            if let Ok(response) = resolver.mx_lookup(&domain).await {
                for mx in response.iter() {
                    records.push(DnsRecord {
                        record_type: "MX".to_string(),
                        value: format!("{} (pri: {})", mx.exchange(), mx.preference()),
                        ttl: response.as_lookup().record_iter().next()
                            .map(|r| r.ttl()).unwrap_or(0),
                    });
                }
            }
            // NS
            if let Ok(response) = resolver.ns_lookup(&domain).await {
                for ns in response.iter() {
                    records.push(DnsRecord {
                        record_type: "NS".to_string(),
                        value: ns.to_string(),
                        ttl: response.as_lookup().record_iter().next()
                            .map(|r| r.ttl()).unwrap_or(0),
                    });
                }
            }
        }
        _ => return Err(AppError::DnsError(format!("不支持的记录类型: {}", record_type))),
    }

    let query_time = start.elapsed().as_secs_f64() * 1000.0;

    Ok(DnsQueryResult {
        domain,
        records,
        query_time_ms: query_time,
    })
}

/// 获取网络信息命令
#[tauri::command]
pub async fn get_network_info() -> AppResult<NetworkInfo> {
    let hostname = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string());

    // 获取本机 IP
    let local_ip = crate::utils::network::get_local_ip().map(|ip| ip.to_string());

    // 获取网络接口信息（通过系统命令）
    let interfaces = get_interfaces().await;

    // 获取默认网关
    let default_gateway = get_default_gateway().await;

    // 获取 DNS 服务器
    let dns_servers = get_dns_servers().await;

    Ok(NetworkInfo {
        local_ip,
        interfaces,
        default_gateway,
        dns_servers,
        hostname,
    })
}

/// 获取网络接口列表
async fn get_interfaces() -> Vec<NetworkInterface> {
    let mut interfaces = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = tokio::process::Command::new("powershell")
            .args(["-Command", "Get-NetIPAddress -AddressFamily IPv4 | Select-Object InterfaceAlias, IPAddress, AddressFamily | ConvertTo-Json"])
            .output()
            .await
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    let items = if json.is_array() {
                        json.as_array().unwrap().clone()
                    } else {
                        vec![json]
                    };
                    for item in items {
                        let name = item["InterfaceAlias"].as_str().unwrap_or("").to_string();
                        let ip = item["IPAddress"].as_str().unwrap_or("").to_string();
                        if !ip.is_empty() && !ip.starts_with("127.") {
                            interfaces.push(NetworkInterface {
                                name,
                                ip,
                                interface_type: "IPv4".to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = tokio::process::Command::new("ip")
            .args(["-4", "-j", "addr", "show"])
            .output()
            .await
        {
            if let Ok(text) = String::from_utf8(output.stdout) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = json.as_array() {
                        for iface in arr {
                            let name = iface["ifname"].as_str().unwrap_or("").to_string();
                            if let Some(addr_info) = iface["addr_info"].as_array() {
                                for addr in addr_info {
                                    let ip = addr["local"].as_str().unwrap_or("").to_string();
                                    if !ip.is_empty() && !ip.starts_with("127.") {
                                        interfaces.push(NetworkInterface {
                                            name: name.clone(),
                                            ip,
                                            interface_type: "IPv4".to_string(),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    interfaces
}

/// 获取默认网关
async fn get_default_gateway() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = tokio::process::Command::new("powershell")
            .args(["-Command", "Get-NetRoute -DestinationPrefix '0.0.0.0/0' | Select-Object -First 1 -ExpandProperty NextHop"])
            .output()
            .await
        {
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !text.is_empty() {
                return Some(text);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(output) = tokio::process::Command::new("ip")
            .args(["route", "show", "default"])
            .output()
            .await
        {
            let text = String::from_utf8_lossy(&output.stdout);
            // 格式: default via 192.168.1.1 dev eth0
            for part in text.split_whitespace() {
                if part.parse::<std::net::IpAddr>().is_ok() {
                    return Some(part.to_string());
                }
            }
        }
    }

    None
}

/// 获取 DNS 服务器
async fn get_dns_servers() -> Vec<String> {
    let mut servers = Vec::new();

    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = tokio::process::Command::new("powershell")
            .args(["-Command", "Get-DnsClientServerAddress -AddressFamily IPv4 | Select-Object -ExpandProperty ServerAddresses | Select-Object -Unique"])
            .output()
            .await
        {
            let text = String::from_utf8_lossy(&output.stdout);
            for line in text.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    servers.push(trimmed.to_string());
                }
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(content) = tokio::fs::read_to_string("/etc/resolv.conf").await {
            for line in content.lines() {
                if line.starts_with("nameserver") {
                    if let Some(server) = line.split_whitespace().nth(1) {
                        servers.push(server.to_string());
                    }
                }
            }
        }
    }

    servers.dedup();
    servers
}
