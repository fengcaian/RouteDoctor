use crate::models::ping::{PingResult, PingStatistics};

/// Calculate statistics from a list of ping results
pub fn calculate_statistics(results: &[PingResult]) -> PingStatistics {
    if results.is_empty() {
        return PingStatistics::default();
    }

    let sent = results.len() as u32;
    let timeouts = results.iter().filter(|r| r.is_timeout).count() as u32;
    let received = sent - timeouts;

    let latencies: Vec<f64> = results
        .iter()
        .filter(|r| !r.is_timeout && r.latency_ms.is_some())
        .map(|r| r.latency_ms.unwrap())
        .collect();

    let loss_rate = if sent > 0 {
        (timeouts as f64 / sent as f64) * 100.0
    } else {
        0.0
    };

    let (min_ms, max_ms, avg_ms, std_dev_ms) = if latencies.is_empty() {
        (0.0, 0.0, 0.0, 0.0)
    } else {
        // Use fold for min/max since f64 doesn't implement Ord
        let min = latencies.iter().copied().fold(f64::INFINITY, |a, b| a.min(b));
        let max = latencies.iter().copied().fold(f64::NEG_INFINITY, |a, b| a.max(b));
        let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;

        // Calculate standard deviation
        let variance = latencies
            .iter()
            .map(|l| (l - avg).powi(2))
            .sum::<f64>() / latencies.len() as f64;
        let std_dev = variance.sqrt();

        (min, max, avg, std_dev)
    };

    // Calculate jitter (average difference between consecutive latencies)
    let jitter_ms = if latencies.len() > 1 {
        let mut total_jitter = 0.0;
        for i in 1..latencies.len() {
            total_jitter += (latencies[i] - latencies[i - 1]).abs();
        }
        total_jitter / (latencies.len() - 1) as f64
    } else {
        0.0
    };

    PingStatistics {
        sent,
        received,
        lost: timeouts,
        loss_rate: round(loss_rate, 2),
        min_ms: round(min_ms, 2),
        max_ms: round(max_ms, 2),
        avg_ms: round(avg_ms, 2),
        jitter_ms: round(jitter_ms, 2),
        std_dev_ms: round(std_dev_ms, 2),
    }
}

/// Round a number to specified decimal places
fn round(value: f64, decimals: u32) -> f64 {
    let multiplier = 10_f64.powi(decimals as i32);
    (value * multiplier).round() / multiplier
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_result(seq: u32, latency: Option<f64>, is_timeout: bool) -> PingResult {
        PingResult {
            seq,
            target: "test".to_string(),
            ip: "127.0.0.1".to_string(),
            latency_ms: latency,
            is_timeout,
            timestamp: 0,
        }
    }

    #[test]
    fn test_calculate_statistics_empty() {
        let stats = calculate_statistics(&[]);
        assert_eq!(stats.sent, 0);
        assert_eq!(stats.received, 0);
        assert_eq!(stats.lost, 0);
    }

    #[test]
    fn test_calculate_statistics_all_success() {
        let results = vec![
            make_result(1, Some(10.0), false),
            make_result(2, Some(20.0), false),
            make_result(3, Some(15.0), false),
        ];
        let stats = calculate_statistics(&results);
        assert_eq!(stats.sent, 3);
        assert_eq!(stats.received, 3);
        assert_eq!(stats.lost, 0);
        assert_eq!(stats.loss_rate, 0.0);
        assert_eq!(stats.min_ms, 10.0);
        assert_eq!(stats.max_ms, 20.0);
    }

    #[test]
    fn test_calculate_statistics_with_timeout() {
        let results = vec![
            make_result(1, Some(10.0), false),
            make_result(2, None, true),
            make_result(3, Some(20.0), false),
        ];
        let stats = calculate_statistics(&results);
        assert_eq!(stats.sent, 3);
        assert_eq!(stats.received, 2);
        assert_eq!(stats.lost, 1);
        assert_eq!(stats.loss_rate, 33.33);
    }
}