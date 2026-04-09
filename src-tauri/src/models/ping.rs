use serde::{Deserialize, Serialize};

/// Result of a single ping operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    /// Sequence number
    pub seq: u32,
    /// Target address (hostname or IP)
    pub target: String,
    /// Resolved IP address
    pub ip: String,
    /// Latency in milliseconds (null if timeout)
    pub latency_ms: Option<f64>,
    /// Whether this ping timed out
    pub is_timeout: bool,
    /// Timestamp of the ping
    pub timestamp: i64,
}

/// Statistics calculated from ping results
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PingStatistics {
    /// Total packets sent
    pub sent: u32,
    /// Total packets received
    pub received: u32,
    /// Total packets lost
    pub lost: u32,
    /// Packet loss rate (percentage)
    pub loss_rate: f64,
    /// Minimum latency (ms)
    pub min_ms: f64,
    /// Maximum latency (ms)
    pub max_ms: f64,
    /// Average latency (ms)
    pub avg_ms: f64,
    /// Jitter (average deviation between consecutive pings)
    pub jitter_ms: f64,
    /// Standard deviation of latency
    pub std_dev_ms: f64,
}

/// Configuration for ping operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingConfig {
    /// Target address
    pub target: String,
    /// Interval between pings (ms)
    pub interval_ms: u32,
    /// Timeout for each ping (ms)
    pub timeout_ms: u32,
    /// Number of pings (None = infinite)
    pub count: Option<u32>,
    /// Packet size in bytes
    pub packet_size: u32,
}