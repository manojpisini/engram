//! Shield agent — security audit & CVE management event loop.

use std::collections::HashSet;
use std::sync::Arc;

use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::json;
use tokio::sync::broadcast;
use tracing::{info, warn, error};

use engram_types::events::{AuditTool, EngramEvent, Severity};
use engram_types::clients::{AgentContext, properties as prop};

use crate::audit_parser::{self};
use crate::cve_deduplicator::{self, DeduplicationResult};
use crate::prompts;

pub async fn run(
    state: Arc<dyn std::any::Any + Send + Sync>,
    mut rx: broadcast::Receiver<EngramEvent>,
) {
    info!("[ShieldAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[ShieldAgent] Error handling event: {e:#}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[ShieldAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[ShieldAgent] Channel closed, shutting down");
                break;
            }
        }
    }
}

fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

async fn handle_event(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    event: &EngramEvent,
) -> Result<()> {
    match event {
        EngramEvent::CiAuditPosted {
            project_id, raw_output, tool, commit_sha, branch,
        } => {
            handle_ci_audit_posted(state, project_id, raw_output, tool, commit_sha, branch).await?;
        }
        EngramEvent::PrMerged {
            repo, pr_number, diff, branch, commit_sha, ..
        } => {
            handle_pr_merged(state, repo, *pr_number, diff, branch, commit_sha).await?;
        }
        EngramEvent::DailyAuditTrigger { project_id } => {
            handle_daily_audit_trigger(state, project_id).await?;
        }
        EngramEvent::SetupComplete { project_id } => {
            info!("[ShieldAgent] SetupComplete — triggering initial daily audit for project {project_id}");
            handle_daily_audit_trigger(state, project_id).await?;
        }
        _ => {}
    }
    Ok(())
}

async fn handle_ci_audit_posted(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
    raw_output: &str,
    tool: &AuditTool,
    commit_sha: &str,
    branch: &str,
) -> Result<()> {
    info!("[ShieldAgent] Processing {tool} audit for project {project_id}");

    let findings = audit_parser::parse_audit_output(tool, raw_output)
        .context("Failed to parse audit output")?;
    info!("[ShieldAgent] Parsed {} findings from {tool}", findings.len());

    // Fetch existing Package IDs from Notion for dedup
    let mut existing_ids: HashSet<String> = HashSet::new();
    if let Some(ctx) = get_ctx(state) {
        let deps_db = &ctx.config.databases.dependencies;
        if !deps_db.is_empty() {
            match ctx.notion.query_database(deps_db, None, None, Some(100)).await {
                Ok(resp) => {
                    if let Some(results) = resp["results"].as_array() {
                        for page in results {
                            if let Some(pid) = page["properties"]["Package ID"]["rich_text"]
                                .as_array()
                                .and_then(|a| a.first())
                                .and_then(|t| t["plain_text"].as_str())
                            {
                                existing_ids.insert(pid.to_string());
                            }
                        }
                        info!("[ShieldAgent] Found {} existing dependency records", existing_ids.len());
                    }
                }
                Err(e) => warn!("[ShieldAgent] Failed to query dependencies: {e}"),
            }
        }
    }

    let DeduplicationResult { new_findings, existing_findings } =
        cve_deduplicator::deduplicate(findings.clone(), &existing_ids);

    info!("[ShieldAgent] Dedup: {} new, {} existing", new_findings.len(), existing_findings.len());

    let now = Utc::now().to_rfc3339();

    // Create Dependency records for new findings
    if let Some(ctx) = get_ctx(state) {
        let deps_db = &ctx.config.databases.dependencies;
        let events_db = &ctx.config.databases.events;

        for finding in &new_findings {
            let pid = cve_deduplicator::package_id_for(finding);
            let cve_csv = finding.cve_ids.join(", ");

            if !deps_db.is_empty() {
                // Claude triage
                let _ai_rec = match ctx.claude.complete(
                    "You are a security expert. Provide a brief triage recommendation for this CVE.",
                    &prompts::cve_triage_prompt(finding, &format!("Project: {project_id}")),
                ).await {
                    Ok(text) => text[..text.len().min(2000)].to_string(),
                    Err(e) => {
                        warn!("[ShieldAgent] Claude triage failed: {e}");
                        format!("Auto-triage pending for {}", finding.package_name)
                    }
                };

                let props = json!({
                    "Package": prop::title(&finding.package_name),
                    "Version": prop::rich_text(&finding.version),
                    "CVE ID": prop::rich_text(&cve_csv),
                    "Severity": prop::select(&finding.severity.to_string()),
                    "Triage Status": prop::select("New"),
                    "Title": prop::rich_text(&format!("{} {}", finding.package_name, finding.version)),
                    "Project ID": prop::rich_text(""),
                    "Commit SHA": prop::rich_text(""),
                });

                match ctx.notion.create_page(deps_db, props).await {
                    Ok(page) => {
                        let id = page["id"].as_str().unwrap_or("?");
                        info!("[ShieldAgent] Created dependency record: {id} ({pid})");
                    }
                    Err(e) => error!("[ShieldAgent] Failed to create dependency record: {e}"),
                }
            }

            // Timeline event for Critical/High
            if matches!(finding.severity, Severity::Critical | Severity::High) && !events_db.is_empty() {
                let props = json!({
                    "Event Title": prop::title(&format!("{} CVE: {} {}", finding.severity, finding.package_name, cve_csv)),
                    "Event Type": prop::rich_text("CVE Found"),
                    "Source Layer": prop::select("Shield"),
                    "Details": prop::rich_text(&format!("Severity: {}", finding.severity)),
                    "Timestamp": prop::date(&now),
                    "Is Milestone": prop::checkbox(true),
                });

                let _ = ctx.notion.create_page(events_db, props).await;
            }
        }

        // Record the Audit Run
        let audit_runs_db = &ctx.config.databases.audit_runs;
        if !audit_runs_db.is_empty() {
            let critical_count = findings.iter().filter(|f| matches!(f.severity, Severity::Critical)).count();
            let high_count = findings.iter().filter(|f| matches!(f.severity, Severity::High)).count();

            let props = json!({
                "Run Name": prop::title(&format!("{tool} audit — {}", &commit_sha[..8.min(commit_sha.len())])),
                "Tool": prop::select(&tool.to_string()),
                "Findings Count": prop::number(findings.len() as f64),
                "Critical Count": prop::number(critical_count as f64),
                "High Count": prop::number(high_count as f64),
                "Commit SHA": prop::rich_text(commit_sha),
                "Branch": prop::rich_text(branch),
                "Project ID": prop::rich_text(""),
            });

            match ctx.notion.create_page(audit_runs_db, props).await {
                Ok(_) => info!("[ShieldAgent] Recorded audit run"),
                Err(e) => warn!("[ShieldAgent] Failed to record audit run: {e}"),
            }
        }
    }

    info!("[ShieldAgent] Audit processing complete");
    Ok(())
}

// ─── PrMerged ───

const LOCKFILE_PATTERNS: &[&str] = &[
    "Cargo.lock", "package-lock.json", "yarn.lock", "pnpm-lock.yaml",
    "Pipfile.lock", "poetry.lock", "requirements.txt", "go.sum",
];

async fn handle_pr_merged(
    _state: &Arc<dyn std::any::Any + Send + Sync>,
    repo: &str,
    pr_number: u64,
    diff: &str,
    branch: &str,
    _commit_sha: &str,
) -> Result<()> {
    let lockfile_changed = LOCKFILE_PATTERNS.iter().any(|p| diff.contains(p));
    if !lockfile_changed {
        return Ok(());
    }
    info!("[ShieldAgent] PR #{pr_number} in {repo} has lockfile changes on {branch}");
    Ok(())
}

// ─── DailyAuditTrigger ───

async fn handle_daily_audit_trigger(
    _state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> Result<()> {
    info!("[ShieldAgent] Daily audit trigger for {project_id}");
    Ok(())
}

/// Detect which audit tools are relevant based on changed lockfiles in a diff.
/// Used by tests now, will be used in PR-triggered auto-audit (future).
#[allow(dead_code)]
fn detect_audit_tools(diff: &str) -> Vec<AuditTool> {
    let mut tools = Vec::new();
    if diff.contains("Cargo.lock") { tools.push(AuditTool::CargoAudit); }
    if diff.contains("package-lock.json") || diff.contains("yarn.lock") {
        tools.push(AuditTool::NpmAudit);
    }
    if diff.contains("Pipfile.lock") || diff.contains("poetry.lock") || diff.contains("requirements.txt") {
        tools.push(AuditTool::PipAudit);
    }
    if !tools.is_empty() { tools.push(AuditTool::OsvScanner); }
    tools
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_audit_tools_cargo() {
        let diff = "Cargo.lock\n+some changes";
        let tools = detect_audit_tools(diff);
        assert!(tools.contains(&AuditTool::CargoAudit));
        assert!(tools.contains(&AuditTool::OsvScanner));
    }

    #[test]
    fn test_detect_audit_tools_none() {
        let diff = "src/main.rs\n+code changes";
        let tools = detect_audit_tools(diff);
        assert!(tools.is_empty());
    }
}
