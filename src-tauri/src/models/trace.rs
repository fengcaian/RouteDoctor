use serde::{Deserialize, Serialize};

/// Probe method for traceroute
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProbeMethod {
    Icmp,
    Udp,
    Tcp,
}

impl std::fmt::Display for ProbeMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProbeMethod::Icmp => write!(f, "icmp"),
            ProbeMethod::Udp => write!(f, "udp"),
            ProbeMethod::Tcp => write!(f, "tcp"),
        }
    }
}

impl Default for ProbeMethod {
    fn default() -> Self {
        ProbeMethod::Icmp
    }
}

/// Result of a traceroute operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracerouteResult {
    /// Target address
    pub target: String,
    /// Resolved target IP
    pub target_ip: String,
    /// List of discovered hops
    pub hops: Vec<HopResult>,
    /// Whether traceroute completed successfully
    pub completed: bool,
    /// Start timestamp
    pub start_time: i64,
    /// End timestamp (null if not completed)
    pub end_time: Option<i64>,
    /// Probe method used
    pub probe_method: ProbeMethod,
}

/// Single hop in traceroute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HopResult {
    /// Hop number (1-30)
    pub hop_number: u32,
    /// IP address of the hop (null if timeout)
    pub ip: Option<String>,
    /// Hostname of the hop (null if not resolved)
    pub hostname: Option<String>,
    /// Latencies from multiple probes (null = timeout)
    pub latencies: Vec<Option<f64>>,
    /// Average latency
    pub avg_latency: Option<f64>,
    /// Packet loss percentage for this hop
    pub packet_loss: f64,
}

/// Configuration for traceroute
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracerouteConfig {
    /// Target address
    pub target: String,
    /// Maximum number of hops
    pub max_hops: u32,
    /// Timeout for each probe (ms)
    pub timeout_ms: u32,
    /// Number of probes per hop
    pub probes_per_hop: u32,
    /// Probe method (ICMP, UDP, TCP)
    pub probe_method: ProbeMethod,
}