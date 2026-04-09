use std::net::IpAddr;

/// Check if an IP address is valid
pub fn is_valid_ip(ip: &str) -> bool {
    ip.parse::<IpAddr>().is_ok()
}

/// Check if a string looks like a hostname
pub fn is_hostname(s: &str) -> bool {
    // Not an IP address and contains at least one dot or is a single word
    !is_valid_ip(s) && (s.contains('.') || s.chars().all(|c| c.is_alphanumeric() || c == '-'))
}

/// Format an IP address for display
pub fn format_ip(ip: &IpAddr) -> String {
    ip.to_string()
}

/// Get the local IP address
pub fn get_local_ip() -> Option<IpAddr> {
    // Try to get local IP by connecting to a public address
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_ip() {
        assert!(is_valid_ip("192.168.1.1"));
        assert!(is_valid_ip("8.8.8.8"));
        assert!(is_valid_ip("::1"));
        assert!(!is_valid_ip("not-an-ip"));
        assert!(!is_valid_ip(""));
    }

    #[test]
    fn test_is_hostname() {
        assert!(is_hostname("google.com"));
        assert!(is_hostname("localhost"));
        assert!(!is_hostname("192.168.1.1"));
    }
}