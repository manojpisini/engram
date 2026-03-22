/// Prompt templates for the Pulse agent's Claude interactions.
///
/// All prompts are formatted at call-time with concrete values and then
/// logged rather than sent to the API (the MCP/Claude integration is
/// stubbed for now).

/// Build the regression impact assessment prompt.
///
/// This is sent to Claude when a benchmark crosses the warning or critical
/// threshold.  Claude should return structured fields that the agent writes
/// into the Regressions Notion database.
///
/// Expected output fields from Claude:
/// - `USER_FACING_IMPACT`  : plain-English description of what users will notice
/// - `ROOT_CAUSE_CATEGORY` : one of Algorithm, Data Structure, I/O, Concurrency, Memory, External Dependency, Configuration
/// - `BISECT_RANGE`        : suggested `git bisect` start..end range
/// - `SEVERITY`            : Critical | High | Medium | Low
pub fn regression_impact_assessment(
    metric_name: &str,
    metric_type: &str,
    current_value: f64,
    baseline_value: f64,
    delta_pct: f64,
    unit: &str,
    commit_sha: &str,
    branch: &str,
    project_id: &str,
    recent_commits: &str,
) -> String {
    format!(
        r#"You are a performance engineering expert analyzing a benchmark regression.

## Regression Data
- **Metric**: {metric_name} ({metric_type})
- **Current value**: {current_value:.3} {unit}
- **Baseline value**: {baseline_value:.3} {unit}
- **Delta**: {delta_pct:+.2}%
- **Commit**: {commit_sha}
- **Branch**: {branch}
- **Project**: {project_id}

## Recent Commits on This Branch
{recent_commits}

## Instructions
Analyze this regression and respond with EXACTLY these four fields in the format shown:

USER_FACING_IMPACT: <one sentence describing what end-users or downstream services will experience>
ROOT_CAUSE_CATEGORY: <one of: Algorithm | Data Structure | I/O | Concurrency | Memory | External Dependency | Configuration>
BISECT_RANGE: <commit_start..commit_end — the most likely range to bisect>
SEVERITY: <Critical | High | Medium | Low>

Guidelines:
- USER_FACING_IMPACT should be concrete ("API responses will be ~200ms slower" not "performance degraded")
- ROOT_CAUSE_CATEGORY should be your best guess given the metric type and recent changes
- BISECT_RANGE should cover the smallest plausible range based on the recent commits
- SEVERITY should account for both the magnitude of the delta and the metric type
  - Critical: >25% regression on latency/throughput, or any production-facing metric crossing SLA
  - High: 15-25% regression or memory/CPU regressions affecting scalability
  - Medium: 5-15% regression on non-critical paths
  - Low: <5% regression or metrics with high natural variance
"#
    )
}

/// Build a prompt for summarising benchmark trends over a time window.
///
/// Used by the weekly digest to produce a narrative about performance changes.
pub fn benchmark_trend_summary(
    project_id: &str,
    period: &str,
    metrics_summary: &str,
    regressions_in_period: usize,
    resolved_in_period: usize,
) -> String {
    format!(
        r#"You are summarizing benchmark performance trends for the engineering digest.

## Context
- **Project**: {project_id}
- **Period**: {period}
- **Regressions detected**: {regressions_in_period}
- **Regressions resolved**: {resolved_in_period}

## Metrics Summary
{metrics_summary}

## Instructions
Write a concise 2-3 sentence summary of performance trends for this period.
Focus on: direction of key metrics, any open regressions, and overall stability.
Use concrete numbers where possible.
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_regression_prompt_contains_fields() {
        let prompt = regression_impact_assessment(
            "parse_json",
            "Latency",
            150.0,
            100.0,
            50.0,
            "ms",
            "abc123def456",
            "main",
            "my-project",
            "abc123 Fix parser\ndef456 Refactor module",
        );
        assert!(prompt.contains("USER_FACING_IMPACT"));
        assert!(prompt.contains("ROOT_CAUSE_CATEGORY"));
        assert!(prompt.contains("BISECT_RANGE"));
        assert!(prompt.contains("SEVERITY"));
        assert!(prompt.contains("parse_json"));
        assert!(prompt.contains("+50.00%"));
    }

    #[test]
    fn test_trend_summary_prompt() {
        let prompt = benchmark_trend_summary(
            "engram",
            "2026-W12",
            "latency_avg: stable at 120ms\nthroughput: +5%",
            2,
            1,
        );
        assert!(prompt.contains("engram"));
        assert!(prompt.contains("2026-W12"));
        assert!(prompt.contains("Regressions detected"));
    }
}
