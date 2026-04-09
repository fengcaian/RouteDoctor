use serde::{Deserialize, Serialize};

/// Result of bandwidth test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthResult {
    /// Download speed in Mbps
    pub download_speed_mbps: f64,
    /// Upload speed in Mbps
    pub upload_speed_mbps: f64,
    /// Latency during test
    pub latency_ms: f64,
    /// Test server used
    pub server: String,
    /// Timestamp of test
    pub timestamp: i64,
}

/// Progress update during bandwidth test
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthProgress {
    /// Current phase
    pub phase: String, // "download", "upload", "idle"
    /// Progress percentage (0-100)
    pub progress: f64,
    /// Current speed in Mbps
    pub current_speed_mbps: f64,
    /// Bytes transferred so far
    pub bytes_transferred: u64,
}