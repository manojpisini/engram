use anyhow::{Context, Result};
use engram_types::events::MetricType;
use serde::{Deserialize, Serialize};

/// A single parsed benchmark measurement, independent of the tool that produced it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkResult {
    /// Human-readable benchmark name (e.g. "parse_large_file")
    pub name: String,
    /// What kind of metric this represents
    pub metric_type: MetricType,
    /// The measured value
    pub value: f64,
    /// Unit of measurement (e.g. "ns", "MB", "req/s")
    pub unit: String,
}

// ─── Criterion (Rust) ────────────────────────────────────────────────

/// Criterion emits one JSON object per benchmark with nested `mean` etc.
/// We accept the "message format" output produced by `cargo criterion --message-format=json`.
#[derive(Debug, Deserialize)]
struct CriterionEntry {
    id: Option<String>,
    reason: Option<String>,
    mean: Option<CriterionEstimate>,
    throughput: Option<Vec<CriterionThroughput>>,
}

#[derive(Debug, Deserialize)]
struct CriterionEstimate {
    estimate: f64,
    unit: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CriterionThroughput {
    per_iteration: Option<f64>,
    unit: Option<String>,
}

/// Parse Criterion JSON benchmark output (newline-delimited JSON messages).
///
/// Each line that has `reason == "benchmark-complete"` is turned into a
/// [`BenchmarkResult`].  Lines without that reason are silently skipped so that
/// the parser is tolerant of Criterion's multi-line output.
pub fn parse_criterion_json(raw: &str) -> Result<Vec<BenchmarkResult>> {
    let mut results = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let entry: CriterionEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(_) => continue, // skip non-JSON lines
        };

        // Only process benchmark-complete messages
        let reason = match &entry.reason {
            Some(r) => r.as_str(),
            None => continue,
        };
        if reason != "benchmark-complete" {
            continue;
        }

        let name = entry.id.unwrap_or_else(|| "unknown".to_string());

        if let Some(mean) = entry.mean {
            let unit = mean.unit.unwrap_or_else(|| "ns".to_string());
            results.push(BenchmarkResult {
                name: name.clone(),
                metric_type: MetricType::Latency,
                value: mean.estimate,
                unit,
            });
        }

        if let Some(throughputs) = entry.throughput {
            for tp in throughputs {
                if let Some(val) = tp.per_iteration {
                    results.push(BenchmarkResult {
                        name: name.clone(),
                        metric_type: MetricType::Throughput,
                        value: val,
                        unit: tp.unit.unwrap_or_else(|| "elem/s".to_string()),
                    });
                }
            }
        }
    }

    if results.is_empty() {
        anyhow::bail!("No benchmark-complete entries found in Criterion output");
    }

    Ok(results)
}

// ─── Hyperfine ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct HyperfineOutput {
    results: Vec<HyperfineResult>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct HyperfineResult {
    command: Option<String>,
    mean: f64,
    stddev: Option<f64>,
    min: Option<f64>,
    max: Option<f64>,
    unit: Option<String>,
}

/// Parse Hyperfine JSON output (`hyperfine --export-json`).
///
/// Hyperfine reports wall-clock timing in seconds by default.  Each command
/// entry produces one [`BenchmarkResult`] keyed on the command string.
pub fn parse_hyperfine_json(raw: &str) -> Result<Vec<BenchmarkResult>> {
    let output: HyperfineOutput =
        serde_json::from_str(raw).context("Failed to parse Hyperfine JSON")?;

    let results: Vec<BenchmarkResult> = output
        .results
        .into_iter()
        .enumerate()
        .map(|(idx, r)| {
            let name = r
                .command
                .unwrap_or_else(|| format!("command_{idx}"));
            BenchmarkResult {
                name,
                metric_type: MetricType::Latency,
                value: r.mean,
                unit: r.unit.unwrap_or_else(|| "s".to_string()),
            }
        })
        .collect();

    if results.is_empty() {
        anyhow::bail!("No results found in Hyperfine output");
    }

    Ok(results)
}

// ─── k6 ──────────────────────────────────────────────────────────────

/// k6 JSON summary (`k6 run --summary-export`).
#[derive(Debug, Deserialize)]
struct K6Summary {
    metrics: K6Metrics,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct K6Metrics {
    http_req_duration: Option<K6Trend>,
    http_reqs: Option<K6Rate>,
    vus: Option<K6Gauge>,
    iterations: Option<K6Rate>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct K6Trend {
    avg: Option<f64>,
    med: Option<f64>,
    #[serde(rename = "p(95)")]
    p95: Option<f64>,
    #[serde(rename = "p(99)")]
    p99: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct K6Rate {
    rate: Option<f64>,
    count: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct K6Gauge {
    value: Option<f64>,
}

/// Parse k6 JSON summary export.
///
/// Extracts latency metrics (avg, p95, p99) and throughput (requests/s).
pub fn parse_k6_json(raw: &str) -> Result<Vec<BenchmarkResult>> {
    let summary: K6Summary =
        serde_json::from_str(raw).context("Failed to parse k6 JSON summary")?;

    let mut results = Vec::new();

    if let Some(ref dur) = summary.metrics.http_req_duration {
        if let Some(avg) = dur.avg {
            results.push(BenchmarkResult {
                name: "http_req_duration_avg".to_string(),
                metric_type: MetricType::Latency,
                value: avg,
                unit: "ms".to_string(),
            });
        }
        if let Some(p95) = dur.p95 {
            results.push(BenchmarkResult {
                name: "http_req_duration_p95".to_string(),
                metric_type: MetricType::Latency,
                value: p95,
                unit: "ms".to_string(),
            });
        }
        if let Some(p99) = dur.p99 {
            results.push(BenchmarkResult {
                name: "http_req_duration_p99".to_string(),
                metric_type: MetricType::Latency,
                value: p99,
                unit: "ms".to_string(),
            });
        }
    }

    if let Some(ref reqs) = summary.metrics.http_reqs {
        if let Some(rate) = reqs.rate {
            results.push(BenchmarkResult {
                name: "http_reqs_rate".to_string(),
                metric_type: MetricType::Throughput,
                value: rate,
                unit: "req/s".to_string(),
            });
        }
    }

    if let Some(ref iters) = summary.metrics.iterations {
        if let Some(rate) = iters.rate {
            results.push(BenchmarkResult {
                name: "iterations_rate".to_string(),
                metric_type: MetricType::Throughput,
                value: rate,
                unit: "iter/s".to_string(),
            });
        }
    }

    if results.is_empty() {
        anyhow::bail!("No metrics found in k6 summary output");
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_criterion_json() {
        let input = r#"{"reason":"benchmark-complete","id":"parse_large","mean":{"estimate":1234.5,"unit":"ns"}}"#;
        let results = parse_criterion_json(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "parse_large");
        assert!((results[0].value - 1234.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_hyperfine_json() {
        let input = r#"{"results":[{"command":"./target/release/app","mean":0.523,"stddev":0.012}]}"#;
        let results = parse_hyperfine_json(input).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "./target/release/app");
        assert!((results[0].value - 0.523).abs() < f64::EPSILON);
    }

    #[test]
    fn test_parse_k6_json() {
        let input = r#"{"metrics":{"http_req_duration":{"avg":120.5,"med":100.0,"p(95)":250.0,"p(99)":400.0},"http_reqs":{"rate":500.0,"count":10000},"vus":{"value":50},"iterations":{"rate":100.0,"count":5000}}}"#;
        let results = parse_k6_json(input).unwrap();
        // avg, p95, p99, http_reqs rate, iterations rate = 5
        assert_eq!(results.len(), 5);
    }
}
