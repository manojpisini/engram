use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};
use engram_types::events::{EngramEvent, BenchmarkStatus, Severity};
use engram_types::clients::{AgentContext, properties as prop};

use crate::benchmark_parser::{self, BenchmarkResult};
use crate::regression_detector;
use crate::prompts;

/// Files whose modification in a PR should trigger benchmark comparison.
const BENCHMARK_CRITICAL_PATTERNS: &[&str] = &[
    "benches/", "src/lib.rs", "src/main.rs", "Cargo.toml", "Cargo.lock",
    "src/parser", "src/engine", "src/core", "src/runtime",
];

const DEFAULT_WARNING_THRESHOLD: f64 = 5.0;
const DEFAULT_CRITICAL_THRESHOLD: f64 = 15.0;
const DEFAULT_PRODUCTION_THRESHOLD: f64 = 25.0;
const DEFAULT_WINDOW_SIZE: usize = 20;

pub async fn run(
    state: Arc<dyn std::any::Any + Send + Sync>,
    mut rx: broadcast::Receiver<EngramEvent>,
) {
    info!("[PulseAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[PulseAgent] Error handling event: {e}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[PulseAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[PulseAgent] Channel closed, shutting down");
                break;
            }
        }
    }
}

/// Extract AgentContext from the Any pointer
fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

async fn handle_event(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    event: &EngramEvent,
) -> anyhow::Result<()> {
    match event {
        EngramEvent::CiBenchmarkPosted {
            project_id, raw_json, commit_sha, branch,
        } => {
            handle_ci_benchmark_posted(state, project_id, raw_json, commit_sha, branch).await?;
        }
        EngramEvent::PrMerged {
            repo, pr_number, diff, branch, commit_sha, title, author, ..
        } => {
            handle_pr_merged(state, repo, *pr_number, diff, branch, commit_sha, title, author).await?;
        }
        EngramEvent::SetupComplete { project_id } => {
            info!("[PulseAgent] SetupComplete — waiting for benchmark data for project {project_id}");
        }
        _ => {}
    }
    Ok(())
}

async fn handle_ci_benchmark_posted(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
    raw_json: &str,
    commit_sha: &str,
    branch: &str,
) -> anyhow::Result<()> {
    info!("[PulseAgent] Processing CiBenchmarkPosted for project={project_id} commit={commit_sha}");

    let results = parse_benchmark_output(raw_json)?;
    info!("[PulseAgent] Parsed {} benchmark result(s)", results.len());

    for result in &results {
        process_single_benchmark(state, project_id, commit_sha, branch, result).await?;
    }

    Ok(())
}

fn parse_benchmark_output(raw_json: &str) -> anyhow::Result<Vec<BenchmarkResult>> {
    if let Ok(results) = benchmark_parser::parse_criterion_json(raw_json) {
        return Ok(results);
    }
    if let Ok(results) = benchmark_parser::parse_hyperfine_json(raw_json) {
        return Ok(results);
    }
    if let Ok(results) = benchmark_parser::parse_k6_json(raw_json) {
        return Ok(results);
    }
    anyhow::bail!("Failed to parse benchmark output as any supported format")
}

async fn process_single_benchmark(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
    commit_sha: &str,
    branch: &str,
    result: &BenchmarkResult,
) -> anyhow::Result<()> {
    let benchmark_id = regression_detector::generate_benchmark_id(project_id, &result.name, commit_sha);

    // Query baseline from Notion
    let mut baseline_value: f64 = 0.0;
    if let Some(ctx) = get_ctx(state) {
        let baselines_db = &ctx.config.databases.performance_baselines;
        if !baselines_db.is_empty() {
            let filter = serde_json::json!({
                "and": [
                    { "property": "Metric Name", "rich_text": { "equals": result.name } },
                    { "property": "Project ID", "rich_text": { "contains": project_id } }
                ]
            });
            match ctx.notion.query_database(baselines_db, Some(filter), None, Some(1)).await {
                Ok(resp) => {
                    if let Some(results_arr) = resp["results"].as_array() {
                        if let Some(first) = results_arr.first() {
                            baseline_value = first["properties"]["Rolling Mean"]["number"]
                                .as_f64().unwrap_or(0.0);
                            info!("[PulseAgent] Found baseline for '{}': {:.3}", result.name, baseline_value);
                        }
                    }
                }
                Err(e) => warn!("[PulseAgent] Failed to query baselines: {e}"),
            }
        }
    }

    // Compute delta
    let delta_pct = regression_detector::compute_delta_pct(result.value, baseline_value);
    let status = regression_detector::detect_regression(
        delta_pct, DEFAULT_WARNING_THRESHOLD, DEFAULT_CRITICAL_THRESHOLD, DEFAULT_PRODUCTION_THRESHOLD,
    );

    info!(
        "[PulseAgent] '{}': value={:.3} {}, baseline={:.3}, delta={:+.2}%, status={}",
        result.name, result.value, result.unit, baseline_value, delta_pct, status
    );

    // Write Benchmark record to Notion
    if let Some(ctx) = get_ctx(state) {
        let benchmarks_db = &ctx.config.databases.benchmarks;
        if !benchmarks_db.is_empty() {
            let _now = chrono::Utc::now().to_rfc3339();
            let props = serde_json::json!({
                "Benchmark Name": prop::title(&result.name),
                "Value": prop::number(result.value),
                "Unit": prop::rich_text(&result.unit),
                "Delta Pct": prop::number(delta_pct),
                "Status": prop::select(&status.to_string()),
                "Commit SHA": prop::rich_text(commit_sha),
                "Branch": prop::rich_text(branch),
                "Project ID": prop::rich_text(project_id),
            });

            match ctx.notion.create_page(benchmarks_db, props).await {
                Ok(page) => {
                    let page_id = page["id"].as_str().unwrap_or("unknown");
                    info!("[PulseAgent] Created Benchmark record: {page_id}");
                }
                Err(e) => error!("[PulseAgent] Failed to create Benchmark record: {e}"),
            }
        }

        // Update rolling baseline
        let baselines_db = &ctx.config.databases.performance_baselines;
        if !baselines_db.is_empty() {
            let historical = vec![result.value];
            let (new_mean, new_stddev) = regression_detector::update_rolling_baseline(&historical, DEFAULT_WINDOW_SIZE);

            let props = serde_json::json!({
                "Baseline ID": prop::title(&format!("{} baseline", result.name)),
                "Metric Name": prop::rich_text(&result.name),
                "Project ID": prop::rich_text(project_id),
                "Rolling Mean": prop::number(new_mean),
                "Rolling Stddev": prop::number(new_stddev),
                "Window Size": prop::number(DEFAULT_WINDOW_SIZE as f64),
            });

            match ctx.notion.create_page(baselines_db, props).await {
                Ok(_) => info!("[PulseAgent] Updated baseline for '{}'", result.name),
                Err(e) => warn!("[PulseAgent] Failed to update baseline: {e}"),
            }
        }

        // If regression, create Regression record and Timeline event
        if matches!(status, BenchmarkStatus::Warning | BenchmarkStatus::Regression | BenchmarkStatus::Critical) {
            handle_regression(ctx, project_id, commit_sha, branch, result, &benchmark_id, baseline_value, delta_pct, &status).await?;
        }

        // Write Timeline event
        let events_db = &ctx.config.databases.events;
        if !events_db.is_empty() {
            let now = chrono::Utc::now().to_rfc3339();
            let props = serde_json::json!({
                "Event Title": prop::title(&format!("Benchmark: {} ({:+.1}%)", result.name, delta_pct)),
                "Event Type": prop::rich_text("Benchmark Posted"),
                "Source Layer": prop::select("Pulse"),
                "Details": prop::rich_text(&format!("Severity: {}", status_to_severity(&status))),
                "Timestamp": prop::date(&now),
                "Project ID": prop::rich_text(project_id),
            });

            match ctx.notion.create_page(events_db, props).await {
                Ok(_) => {}
                Err(e) => warn!("[PulseAgent] Failed to create timeline event: {e}"),
            }
        }
    }

    Ok(())
}

async fn handle_regression(
    ctx: &AgentContext,
    project_id: &str,
    commit_sha: &str,
    branch: &str,
    result: &BenchmarkResult,
    benchmark_id: &str,
    baseline_value: f64,
    delta_pct: f64,
    status: &BenchmarkStatus,
) -> anyhow::Result<()> {
    let severity = status_to_severity(status);
    let _regression_id = format!("REG-{benchmark_id}");

    // Claude impact assessment
    let prompt = prompts::regression_impact_assessment(
        &result.name, &result.metric_type.to_string(),
        result.value, baseline_value, delta_pct, &result.unit,
        commit_sha, branch, project_id,
        "<recent commits fetched from Git>",
    );

    let _impact_text = match ctx.claude.complete(
        "You are a performance engineering expert. Analyze this benchmark regression and provide a concise impact assessment.",
        &prompt,
    ).await {
        Ok(text) => {
            info!("[PulseAgent] Claude assessment received ({} chars)", text.len());
            text
        }
        Err(e) => {
            warn!("[PulseAgent] Claude assessment failed: {e}");
            format!("Regression detected: {} {:+.2}% from baseline", result.name, delta_pct)
        }
    };

    // Create Regression record
    let regressions_db = &ctx.config.databases.regressions;
    if !regressions_db.is_empty() {
        let _now = chrono::Utc::now().to_rfc3339();
        let props = serde_json::json!({
            "Metric Name": prop::title(&format!("Regression: {} ({:+.1}%)", result.name, delta_pct)),
            "Delta Pct": prop::number(delta_pct),
            "Baseline Value": prop::number(baseline_value),
            "Current Value": prop::number(result.value),
            "Severity": prop::select(&severity.to_string()),
            "Status": prop::select("Open"),
            "Commit SHA": prop::rich_text(commit_sha),
            "Project ID": prop::rich_text(project_id),
        });

        match ctx.notion.create_page(regressions_db, props).await {
            Ok(page) => {
                let page_id = page["id"].as_str().unwrap_or("unknown");
                info!("[PulseAgent] Created Regression record: {page_id}");
            }
            Err(e) => error!("[PulseAgent] Failed to create Regression record: {e}"),
        }
    }

    Ok(())
}

fn status_to_severity(status: &BenchmarkStatus) -> Severity {
    match status {
        BenchmarkStatus::Critical => Severity::Critical,
        BenchmarkStatus::Regression => Severity::High,
        BenchmarkStatus::Warning => Severity::Medium,
        BenchmarkStatus::Normal => Severity::Info,
    }
}

// ─── PrMerged handler ────────────────────────────────────────────────

async fn handle_pr_merged(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    _repo: &str,
    pr_number: u64,
    diff: &str,
    _branch: &str,
    _commit_sha: &str,
    title: &str,
    author: &str,
) -> anyhow::Result<()> {
    info!("[PulseAgent] Processing PrMerged: #{pr_number} '{title}' by {author}");

    let dominated = diff_touches_critical_files(diff);
    if !dominated {
        info!("[PulseAgent] PR #{pr_number} does not touch benchmark-critical files — skipping");
        return Ok(());
    }

    info!("[PulseAgent] PR #{pr_number} touches benchmark-critical files");

    if let Some(ctx) = get_ctx(state) {
        let events_db = &ctx.config.databases.events;
        if !events_db.is_empty() {
            let now = chrono::Utc::now().to_rfc3339();
            let props = serde_json::json!({
                "Event Title": prop::title(&format!("PR #{pr_number} merged (benchmark-critical)")),
                "Event Type": prop::rich_text("PR Merged"),
                "Source Layer": prop::select("Pulse"),
                "Timestamp": prop::date(&now),
            });
            let _ = ctx.notion.create_page(events_db, props).await;
        }
    }

    Ok(())
}

fn diff_touches_critical_files(diff: &str) -> bool {
    for line in diff.lines() {
        let path = if let Some(stripped) = line.strip_prefix("+++ b/") {
            stripped
        } else if let Some(stripped) = line.strip_prefix("--- a/") {
            stripped
        } else {
            continue;
        };
        for pattern in BENCHMARK_CRITICAL_PATTERNS {
            if path.starts_with(pattern) || path.contains(pattern) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_touches_critical_files_positive() {
        let diff = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,3 +1,4 @@\n+use something;\n";
        assert!(diff_touches_critical_files(diff));
    }

    #[test]
    fn test_diff_touches_critical_files_negative() {
        let diff = "--- a/docs/README.md\n+++ b/docs/README.md\n@@ -1 +1 @@\n-old\n+new\n";
        assert!(!diff_touches_critical_files(diff));
    }

    #[test]
    fn test_status_to_severity() {
        assert_eq!(status_to_severity(&BenchmarkStatus::Critical), Severity::Critical);
        assert_eq!(status_to_severity(&BenchmarkStatus::Regression), Severity::High);
        assert_eq!(status_to_severity(&BenchmarkStatus::Warning), Severity::Medium);
        assert_eq!(status_to_severity(&BenchmarkStatus::Normal), Severity::Info);
    }
}
