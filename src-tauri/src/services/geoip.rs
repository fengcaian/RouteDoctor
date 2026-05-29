use std::collections::HashMap;
use std::net::IpAddr;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// GeoIP information for a single IP address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeoInfo {
    pub ip: String,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub region: Option<String>,
    pub city: Option<String>,
    pub isp: Option<String>,
    pub org: Option<String>,
    pub asn: Option<String>,
    pub as_name: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
}

/// Cache entry with an expiry timestamp.
struct CacheEntry {
    info: Option<GeoInfo>, // None means "private/reserved IP, skip future lookups"
    expires_at: i64,       // unix epoch seconds
}

/// Global GeoIP cache: IP string -> cached result.
static CACHE: Lazy<RwLock<HashMap<String, CacheEntry>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Global HTTP client for GeoIP API calls.
static HTTP: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
});

const CACHE_TTL_SECS: i64 = 24 * 3600; // 24 hours

/// Check if an IP address is private, loopback, or link-local (no point
/// querying a public GeoIP service for these).
pub fn is_private(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private() || v4.is_loopback() || v4.is_link_local() || v4.is_broadcast()
                || v4.is_documentation() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            // Loopback, link-local, unique-local, unspecified
            v6.is_loopback() || v6.is_unspecified()
                || matches!(v6.segments()[0], 0xfc00..=0xfdff) // unique-local
                || matches!(v6.segments()[0], 0xfe80)          // link-local
        }
    }
}

/// Lookup GeoIP for a single IP. Returns `None` for private/reserved IPs or
/// on any failure (silently — the UI will just not show geo data).
pub async fn lookup_one(ip: &IpAddr) -> Option<GeoInfo> {
    let ip_str = ip.to_string();

    // Check cache first
    {
        let cache = CACHE.read().await;
        if let Some(entry) = cache.get(&ip_str) {
            let now = chrono::Utc::now().timestamp();
            if now < entry.expires_at {
                return entry.info.clone();
            }
        }
    }

    // Private IPs — cache as None so we never hit the API again
    if is_private(ip) {
        let mut cache = CACHE.write().await;
        cache.insert(
            ip_str.clone(),
            CacheEntry {
                info: None,
                expires_at: i64::MAX,
            },
        );
        return None;
    }

    // Query ip-api.com (free, no token needed)
    let result = query_ip_api(&ip_str).await;

    let now = chrono::Utc::now().timestamp();
    let mut cache = CACHE.write().await;
    cache.insert(
        ip_str,
        CacheEntry {
            info: result.clone(),
            expires_at: now + CACHE_TTL_SECS,
        },
    );

    result
}

/// Lookup GeoIP for multiple IPs in batch (uses ip-api.com batch endpoint).
pub async fn lookup_batch(ips: &[IpAddr]) -> HashMap<String, GeoInfo> {
    let mut results = HashMap::new();

    // Filter out private IPs and already-cached entries
    let mut to_query: Vec<String> = Vec::new();
    let now = chrono::Utc::now().timestamp();

    {
        let cache = CACHE.read().await;
        for ip in ips {
            let ip_str = ip.to_string();
            if is_private(ip) {
                continue;
            }
            if let Some(entry) = cache.get(&ip_str) {
                if now < entry.expires_at {
                    if let Some(ref info) = entry.info {
                        results.insert(ip_str, info.clone());
                    }
                    continue;
                }
            }
            to_query.push(ip_str);
        }
    }

    if to_query.is_empty() {
        return results;
    }

    // ip-api.com batch supports up to 100 IPs per request
    for chunk in to_query.chunks(100) {
        if let Ok(batch_results) = query_ip_api_batch(chunk).await {
            let mut cache = CACHE.write().await;
            let now = chrono::Utc::now().timestamp();
            for (ip_str, geo) in batch_results {
                cache.insert(
                    ip_str.clone(),
                    CacheEntry {
                        info: Some(geo.clone()),
                        expires_at: now + CACHE_TTL_SECS,
                    },
                );
                results.insert(ip_str, geo);
            }
        }
    }

    results
}

/// Query ip-api.com for a single IP.
async fn query_ip_api(ip: &str) -> Option<GeoInfo> {
    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct IpApiResponse {
        status: String,
        country: Option<String>,
        #[serde(rename = "countryCode")]
        country_code: Option<String>,
        regionName: Option<String>,
        city: Option<String>,
        isp: Option<String>,
        org: Option<String>,
        #[serde(rename = "as")]
        asn: Option<String>,
        lat: Option<f64>,
        lon: Option<f64>,
    }

    let url = format!("http://ip-api.com/json/{}?fields=status,country,countryCode,regionName,city,isp,org,as,lat,lon", ip);

    let resp = HTTP.get(&url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }

    let data: IpApiResponse = resp.json().await.ok()?;
    if data.status != "success" {
        return None;
    }

    // Split "AS15169 Google LLC" into ASN number and name
    let (asn, as_name) = split_asn(data.asn.as_deref());

    Some(GeoInfo {
        ip: ip.to_string(),
        country: data.country,
        country_code: data.country_code,
        region: data.regionName,
        city: data.city,
        isp: data.isp,
        org: data.org,
        asn,
        as_name,
        lat: data.lat,
        lon: data.lon,
    })
}

/// Query ip-api.com batch endpoint for multiple IPs.
async fn query_ip_api_batch(ips: &[String]) -> Result<HashMap<String, GeoInfo>, ()> {
    #[derive(Deserialize)]
    #[allow(non_snake_case)]
    struct IpApiResponse {
        query: Option<String>,
        status: Option<String>,
        country: Option<String>,
        #[serde(rename = "countryCode")]
        country_code: Option<String>,
        regionName: Option<String>,
        city: Option<String>,
        isp: Option<String>,
        org: Option<String>,
        #[serde(rename = "as")]
        asn: Option<String>,
        lat: Option<f64>,
        lon: Option<f64>,
    }

    // Build batch request body: each entry is an object with "query" field
    let body: Vec<serde_json::Value> = ips
        .iter()
        .map(|ip| serde_json::json!({ "query": ip, "fields": "query,status,country,countryCode,regionName,city,isp,org,as,lat,lon" }))
        .collect();

    let resp = HTTP
        .post("http://ip-api.com/batch")
        .json(&body)
        .send()
        .await
        .map_err(|_| ())?;

    if !resp.status().is_success() {
        return Err(());
    }

    let items: Vec<IpApiResponse> = resp.json().await.map_err(|_| ())?;
    let mut results = HashMap::new();

    for data in items {
        let ip_str = data.query.unwrap_or_default();
        if ip_str.is_empty() || data.status.as_deref() != Some("success") {
            continue;
        }
        let (asn, as_name) = split_asn(data.asn.as_deref());
        results.insert(
            ip_str.clone(),
            GeoInfo {
                ip: ip_str,
                country: data.country,
                country_code: data.country_code,
                region: data.regionName,
                city: data.city,
                isp: data.isp,
                org: data.org,
                asn,
                as_name,
                lat: data.lat,
                lon: data.lon,
            },
        );
    }

    Ok(results)
}

/// Split "AS15169 Google LLC" into (Some("AS15169"), Some("Google LLC")).
fn split_asn(as_field: Option<&str>) -> (Option<String>, Option<String>) {
    match as_field {
        Some(s) if s.starts_with("AS") => {
            if let Some(space) = s.find(' ') {
                (
                    Some(s[..space].to_string()),
                    Some(s[space + 1..].to_string()),
                )
            } else {
                (Some(s.to_string()), None)
            }
        }
        Some(s) => (None, Some(s.to_string())),
        None => (None, None),
    }
}
