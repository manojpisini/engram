use engram_types::events::BenchmarkStatus;

/// Compute the percentage change from baseline to current.
///
/// Formula: `(current - baseline) / baseline * 100`
///
/// A positive result means the metric *increased* (e.g., slower latency),
/// a negative result means it *decreased* (e.g., faster latency).
/// Returns 0.0 when the baseline is zero to avoid division-by-zero.
pub fn compute_delta_pct(current: f64, baseline: f64) -> f64 {
    if baseline == 0.0 {
        return 0.0;
    }
    (current - baseline) / baseline * 100.0
}

/// Classify a delta percentage against the three-tier threshold scheme.
///
/// | Range                          | Status       |
/// |--------------------------------|-------------|
/// | abs(delta) <= warning          | Normal      |
/// | warning < abs(delta) <= critical | Warning   |
/// | critical < abs(delta) <= production | Regression |
/// | abs(delta) > production        | Critical    |
pub fn detect_regression(
    delta_pct: f64,
    warning_threshold: f64,
    critical_threshold: f64,
    production_threshold: f64,
) -> BenchmarkStatus {
    BenchmarkStatus::from_delta(delta_pct, warning_threshold, critical_threshold, production_threshold)
}

/// Compute the rolling mean and standard deviation over a window of recent values.
///
/// Returns `(mean, stddev)`.  If `values` is empty, returns `(0.0, 0.0)`.
/// The window is taken from the *end* of the slice (most recent values).
pub fn update_rolling_baseline(values: &[f64], window_size: usize) -> (f64, f64) {
    if values.is_empty() {
        return (0.0, 0.0);
    }

    let start = values.len().saturating_sub(window_size);
    let window = &values[start..];
    let n = window.len() as f64;

    let mean = window.iter().sum::<f64>() / n;

    let variance = window.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n;
    let stddev = variance.sqrt();

    (mean, stddev)
}

/// Generate a deterministic Benchmark ID.
///
/// Format: `{project}-{metric_name}-{short_sha}` where short_sha is the first 8
/// characters of the commit SHA.
pub fn generate_benchmark_id(project: &str, metric_name: &str, commit_sha: &str) -> String {
    let short_sha = if commit_sha.len() >= 8 {
        &commit_sha[..8]
    } else {
        commit_sha
    };
    format!("{project}-{metric_name}-{short_sha}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_delta_pct_increase() {
        // 110 vs baseline 100 => +10%
        let delta = compute_delta_pct(110.0, 100.0);
        assert!((delta - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_delta_pct_decrease() {
        // 90 vs baseline 100 => -10%
        let delta = compute_delta_pct(90.0, 100.0);
        assert!((delta - (-10.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_delta_pct_zero_baseline() {
        assert_eq!(compute_delta_pct(42.0, 0.0), 0.0);
    }

    #[test]
    fn test_detect_regression_normal() {
        let status = detect_regression(2.0, 5.0, 15.0, 25.0);
        assert_eq!(status, BenchmarkStatus::Normal);
    }

    #[test]
    fn test_detect_regression_warning() {
        let status = detect_regression(8.0, 5.0, 15.0, 25.0);
        assert_eq!(status, BenchmarkStatus::Warning);
    }

    #[test]
    fn test_detect_regression_regression() {
        let status = detect_regression(20.0, 5.0, 15.0, 25.0);
        assert_eq!(status, BenchmarkStatus::Regression);
    }

    #[test]
    fn test_detect_regression_critical() {
        let status = detect_regression(30.0, 5.0, 15.0, 25.0);
        assert_eq!(status, BenchmarkStatus::Critical);
    }

    #[test]
    fn test_detect_regression_negative_delta() {
        // Negative deltas are also checked by absolute value
        let status = detect_regression(-30.0, 5.0, 15.0, 25.0);
        assert_eq!(status, BenchmarkStatus::Critical);
    }

    #[test]
    fn test_update_rolling_baseline_basic() {
        let values = vec![100.0, 102.0, 98.0, 101.0, 99.0];
        let (mean, stddev) = update_rolling_baseline(&values, 5);
        assert!((mean - 100.0).abs() < 0.01);
        assert!(stddev > 0.0);
    }

    #[test]
    fn test_update_rolling_baseline_windowed() {
        // Window of 3 takes last 3 values: [101, 99, 100]
        let values = vec![200.0, 300.0, 101.0, 99.0, 100.0];
        let (mean, _stddev) = update_rolling_baseline(&values, 3);
        assert!((mean - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_update_rolling_baseline_empty() {
        let (mean, stddev) = update_rolling_baseline(&[], 10);
        assert_eq!(mean, 0.0);
        assert_eq!(stddev, 0.0);
    }

    #[test]
    fn test_generate_benchmark_id() {
        let id = generate_benchmark_id("myproject", "latency_avg", "abcdef1234567890");
        assert_eq!(id, "myproject-latency_avg-abcdef12");
    }

    #[test]
    fn test_generate_benchmark_id_short_sha() {
        let id = generate_benchmark_id("proj", "mem", "abc");
        assert_eq!(id, "proj-mem-abc");
    }
}
