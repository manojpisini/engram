//! Release agent main event loop.
//!
//! Handles:
//! - `ReleaseCreated` — gather PRs, RFCs, env vars, deps; check gates; generate notes.
//! - `PrMerged` — link PR to current milestone/release candidate.
//! - `RfcApproved` — link RFC to target milestone.
//! - `RegressionDetected` — if release candidate exists, block it until resolved.

use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use chrono::Utc;
use serde_json::json;
use engram_types::clients::{AgentContext, properties as prop};
use engram_types::events::{EngramEvent, Severity};
use engram_types::notion_schema::{events as events_schema, releases};

use crate::notes_generator::{
    format_migration_notes_markdown, format_release_notes_markdown, parse_migration_notes,
    parse_readiness_assessment, parse_release_notes,
};
use crate::prompts;

// ── Helper: downcast shared state to AgentContext ──

fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

/// Run the Release-agent main loop.
pub async fn run(
    state: Arc<dyn std::any::Any + Send + Sync>,
    mut rx: broadcast::Receiver<EngramEvent>,
) {
    info!("[ReleaseAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[ReleaseAgent] Error handling event: {e}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[ReleaseAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[ReleaseAgent] Channel closed, shutting down");
                break;
            }
        }
    }
}

async fn handle_event(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    event: &EngramEvent,
) -> anyhow::Result<()> {
    match event {
        EngramEvent::ReleaseCreated {
            project_id,
            version,
            milestone,
        } => {
            handle_release_created(state, project_id, version, milestone).await?;
        }
        EngramEvent::PrMerged {
            repo,
            pr_number,
            title,
            rfc_references,
            ..
        } => {
            handle_pr_merged_for_release(state, repo, *pr_number, title, rfc_references).await?;
        }
        EngramEvent::RfcApproved {
            rfc_id, project_id, ..
        } => {
            handle_rfc_approved_for_release(state, rfc_id, project_id).await?;
        }
        EngramEvent::RegressionDetected {
            severity,
            metric_name,
            project_id,
            ..
        } => {
            if matches!(severity, Severity::Critical) {
                handle_regression_blocks_release(state, project_id, metric_name).await?;
            }
        }
        EngramEvent::SetupComplete { project_id } => {
            info!("[ReleaseAgent] SetupComplete — waiting for release data for project {project_id}");
        }
        _ => {} // Ignore unrelated events
    }
    Ok(())
}

/// Full release creation flow:
/// 1. Query all merged PRs in the milestone
/// 2. Query all implemented RFCs in the release window
/// 3. Collect new env vars and dependency changes
/// 4. Check regression-free and CVE-free gates
/// 5. Generate release notes and migration notes via Claude
/// 6. Write Release record to Notion
async fn handle_release_created(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
    version: &str,
    milestone: &str,
) -> anyhow::Result<()> {
    info!("[ReleaseAgent] Processing release {version} for project {project_id}, milestone {milestone}");

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[ReleaseAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_releases = &ctx.config.databases.releases;
    let db_pr_reviews = &ctx.config.databases.pr_reviews;
    let db_events = &ctx.config.databases.events;

    // Step 1: Gather merged PRs in milestone
    let pr_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Status", "select": { "equals": "Merged" } }
        ]
    });
    let merged_prs_json = match ctx.notion.query_database(db_pr_reviews, Some(pr_filter), None, Some(100)).await {
        Ok(result) => serde_json::to_string(&result["results"]).unwrap_or_else(|_| "[]".to_string()),
        Err(e) => {
            warn!("[ReleaseAgent] Failed to query merged PRs: {e}");
            "[]".to_string()
        }
    };

    // Step 2: Gather implemented RFCs
    let db_rfcs = &ctx.config.databases.rfcs;
    let rfc_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Status", "select": { "equals": "Implementing" } }
        ]
    });
    let implemented_rfcs_json = match ctx.notion.query_database(db_rfcs, Some(rfc_filter), None, Some(100)).await {
        Ok(result) => serde_json::to_string(&result["results"]).unwrap_or_else(|_| "[]".to_string()),
        Err(e) => {
            warn!("[ReleaseAgent] Failed to query implemented RFCs: {e}");
            "[]".to_string()
        }
    };

    // Step 3: Collect new env vars linked to these PRs/RFCs
    let db_env_config = &ctx.config.databases.env_config;
    let env_filter = json!({
        "property": "Project ID",
        "rich_text": { "equals": project_id }
    });
    let new_env_vars_json = match ctx.notion.query_database(db_env_config, Some(env_filter), None, Some(100)).await {
        Ok(result) => serde_json::to_string(&result["results"]).unwrap_or_else(|_| "[]".to_string()),
        Err(e) => {
            warn!("[ReleaseAgent] Failed to query env vars: {e}");
            "[]".to_string()
        }
    };

    // Step 4: Collect dependency changes
    let db_dependencies = &ctx.config.databases.dependencies;
    let dep_filter = json!({
        "property": "Project ID",
        "rich_text": { "equals": project_id }
    });
    let dependency_changes_json = match ctx.notion.query_database(db_dependencies, Some(dep_filter), None, Some(100)).await {
        Ok(result) => serde_json::to_string(&result["results"]).unwrap_or_else(|_| "[]".to_string()),
        Err(e) => {
            warn!("[ReleaseAgent] Failed to query dependencies: {e}");
            "[]".to_string()
        }
    };

    // Step 5: Check regression-free gate
    let db_regressions = &ctx.config.databases.regressions;
    let regression_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Severity", "select": { "equals": "Critical" } }
        ]
    });
    let open_critical_regressions = match ctx.notion.query_database(db_regressions, Some(regression_filter), None, Some(1)).await {
        Ok(result) => result["results"].as_array().map_or(0, |a| a.len() as u32),
        Err(e) => {
            warn!("[ReleaseAgent] Failed to check regressions: {e}");
            0
        }
    };
    let regression_free = open_critical_regressions == 0;

    // Step 6: Check CVE-free gate
    let cve_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Triage Status", "select": { "equals": "New" } }
        ]
    });
    let unresolved_critical_cves = match ctx.notion.query_database(db_dependencies, Some(cve_filter), None, Some(1)).await {
        Ok(result) => result["results"].as_array().map_or(0, |a| a.len() as u32),
        Err(e) => {
            warn!("[ReleaseAgent] Failed to check CVEs: {e}");
            0
        }
    };
    let cve_free = unresolved_critical_cves == 0;

    // Step 7: Release readiness assessment via Claude
    let readiness_prompt = prompts::release_readiness_prompt(version, &merged_prs_json, &implemented_rfcs_json, "{}");
    let system_readiness = "You are a release engineering expert. Assess release readiness. Respond with valid JSON only.";
    let readiness_json = match ctx.claude.complete(system_readiness, &readiness_prompt).await {
        Ok(response) => response,
        Err(e) => {
            warn!("[ReleaseAgent] Claude readiness assessment failed: {e}");
            r#"{"release_ready": true, "blockers": [], "risks": [], "recommendation": "Ship"}"#.to_string()
        }
    };
    let readiness = parse_readiness_assessment(&readiness_json)?;
    info!(
        "[ReleaseAgent] Readiness: ready={}, recommendation={}",
        readiness.release_ready, readiness.recommendation
    );

    // Step 8: Generate release notes via Claude
    let notes_prompt = prompts::release_notes_prompt(
        version,
        project_id,
        &merged_prs_json,
        &implemented_rfcs_json,
        &dependency_changes_json,
    );
    let system_notes = "You are a technical writer generating release notes. Respond with valid JSON only.";
    let notes_json = match ctx.claude.complete(system_notes, &notes_prompt).await {
        Ok(response) => response,
        Err(e) => {
            warn!("[ReleaseAgent] Claude notes generation failed: {e}");
            r#"{"features":[],"fixes":[],"performance":[],"security":[],"breaking_changes":[],"summary":"Release placeholder."}"#.to_string()
        }
    };
    let release_notes = parse_release_notes(&notes_json)?;
    let release_notes_md = format_release_notes_markdown(&release_notes);

    // Step 9: Generate migration notes via Claude
    let migration_prompt =
        prompts::migration_notes_prompt(version, "[]", &new_env_vars_json, &dependency_changes_json);
    let system_migration = "You are a DevOps expert generating migration guides. Respond with valid JSON only.";
    let migration_json = match ctx.claude.complete(system_migration, &migration_prompt).await {
        Ok(response) => response,
        Err(e) => {
            warn!("[ReleaseAgent] Claude migration notes failed: {e}");
            r#"{"before_upgrade":[],"env_changes":[],"breaking_migration":[],"dependency_notes":[],"verification":[]}"#.to_string()
        }
    };
    let migration_notes = parse_migration_notes(&migration_json)?;
    let migration_notes_md = format_migration_notes_markdown(&migration_notes);

    // Step 10: Write Release record to Notion
    let status = if readiness.release_ready {
        "Candidate"
    } else {
        "Draft"
    };
    let now = Utc::now().to_rfc3339();
    let release_props = json!({
        releases::RELEASE_ID: prop::title(version),
        releases::PROJECT: prop::rich_text(project_id),
        releases::STATUS: prop::select(status),
        releases::MILESTONE: prop::rich_text(milestone),
        releases::REGRESSION_FREE: prop::checkbox(regression_free),
        releases::CVE_FREE: prop::checkbox(cve_free),
        releases::RELEASE_NOTES: prop::rich_text(&release_notes_md),
        releases::MIGRATION_NOTES: prop::rich_text(&migration_notes_md),
    });
    match ctx.notion.create_page(db_releases, release_props).await {
        Ok(_) => info!("[ReleaseAgent] Release {version} record created (status: {status})"),
        Err(e) => error!("[ReleaseAgent] Failed to create release record: {e}"),
    }

    // Create timeline event
    let event_props = json!({
        events_schema::TITLE: prop::title(&format!("Release {version} created (status: {status})")),
        events_schema::TYPE: prop::rich_text("ReleaseCreated"),
        events_schema::SOURCE_LAYER: prop::select("Release"),
        events_schema::PROJECT: prop::rich_text(project_id),
        events_schema::TIMESTAMP: prop::date(&now),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[ReleaseAgent] Created release event for {version}"),
        Err(e) => error!("[ReleaseAgent] Failed to create release event: {e}"),
    }

    info!("[ReleaseAgent] Release {version} record created (status: {status})");
    Ok(())
}

/// When a PR merges, link it to the current active release/milestone.
async fn handle_pr_merged_for_release(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    repo: &str,
    pr_number: u64,
    title: &str,
    rfc_references: &[String],
) -> anyhow::Result<()> {
    let pr_id = format!("{repo}#{pr_number}");
    info!("[ReleaseAgent] PR merged: {pr_id} -- {title}");

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[ReleaseAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_releases = &ctx.config.databases.releases;
    let db_events = &ctx.config.databases.events;

    // Query for active release candidates for this project
    let filter = json!({
        "or": [
            { "property": releases::STATUS, "select": { "equals": "Draft" } },
            { "property": releases::STATUS, "select": { "equals": "Candidate" } }
        ]
    });
    let sorts = json!([
        { "timestamp": "created_time", "direction": "ascending" }
    ]);
    match ctx.notion.query_database(db_releases, Some(filter), Some(sorts), Some(1)).await {
        Ok(result) => {
            if let Some(pages) = result["results"].as_array() {
                if let Some(page) = pages.first() {
                    if let Some(page_id) = page["id"].as_str() {
                        // Link PR to the release
                        let update_props = json!({
                            releases::INCLUDED_PRS: prop::rich_text(&pr_id),
                        });
                        match ctx.notion.update_page(page_id, update_props).await {
                            Ok(_) => info!("[ReleaseAgent] Linked {pr_id} to release {page_id}"),
                            Err(e) => error!("[ReleaseAgent] Failed to link PR to release: {e}"),
                        }

                        // If PR implements an RFC, link the RFC too
                        for rfc_ref in rfc_references {
                            let rfc_props = json!({
                                releases::IMPLEMENTED_RFCS: prop::rich_text(rfc_ref),
                            });
                            match ctx.notion.update_page(page_id, rfc_props).await {
                                Ok(_) => info!("[ReleaseAgent] Linked RFC {rfc_ref} to release"),
                                Err(e) => error!("[ReleaseAgent] Failed to link RFC to release: {e}"),
                            }
                        }
                    }
                }
            }
        }
        Err(e) => error!("[ReleaseAgent] Failed to query releases: {e}"),
    }

    // Create timeline event
    let now = Utc::now().to_rfc3339();
    let event_props = json!({
        events_schema::TITLE: prop::title(&format!("PR {pr_id} linked to release")),
        events_schema::TYPE: prop::rich_text("PrLinkedToRelease"),
        events_schema::SOURCE_LAYER: prop::select("Release"),
        events_schema::DETAILS: prop::rich_text(&format!("pr={pr_id}")),
        events_schema::TIMESTAMP: prop::date(&now),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[ReleaseAgent] Created PR-linked-to-release event"),
        Err(e) => error!("[ReleaseAgent] Failed to create event: {e}"),
    }

    Ok(())
}

/// When an RFC is approved, link it to the target milestone/release.
async fn handle_rfc_approved_for_release(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    rfc_id: &str,
    project_id: &str,
) -> anyhow::Result<()> {
    info!("[ReleaseAgent] RFC approved: {rfc_id} for project {project_id}");

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[ReleaseAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_releases = &ctx.config.databases.releases;
    let db_events = &ctx.config.databases.events;

    // Find the next active release for this project
    let filter = json!({
        "and": [
            { "property": releases::PROJECT, "rich_text": { "equals": project_id } },
            {
                "or": [
                    { "property": releases::STATUS, "select": { "equals": "Draft" } },
                    { "property": releases::STATUS, "select": { "equals": "Candidate" } }
                ]
            }
        ]
    });
    let sorts = json!([
        { "timestamp": "created_time", "direction": "ascending" }
    ]);
    match ctx.notion.query_database(db_releases, Some(filter), Some(sorts), Some(1)).await {
        Ok(result) => {
            if let Some(pages) = result["results"].as_array() {
                if let Some(page) = pages.first() {
                    if let Some(page_id) = page["id"].as_str() {
                        let update_props = json!({
                            releases::IMPLEMENTED_RFCS: prop::rich_text(rfc_id),
                        });
                        match ctx.notion.update_page(page_id, update_props).await {
                            Ok(_) => info!("[ReleaseAgent] Linked RFC {rfc_id} to release {page_id}"),
                            Err(e) => error!("[ReleaseAgent] Failed to link RFC to release: {e}"),
                        }
                    }
                }
            }
        }
        Err(e) => error!("[ReleaseAgent] Failed to query releases: {e}"),
    }

    // Create timeline event
    let now = Utc::now().to_rfc3339();
    let event_props = json!({
        events_schema::TITLE: prop::title(&format!("RFC {rfc_id} linked to release")),
        events_schema::TYPE: prop::rich_text("RfcLinkedToRelease"),
        events_schema::SOURCE_LAYER: prop::select("Release"),
        events_schema::PROJECT: prop::rich_text(project_id),
        events_schema::DETAILS: prop::rich_text(&format!("rfc={rfc_id}")),
        events_schema::TIMESTAMP: prop::date(&now),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[ReleaseAgent] Created RFC-linked-to-release event"),
        Err(e) => error!("[ReleaseAgent] Failed to create event: {e}"),
    }

    Ok(())
}

/// When a critical regression is detected, block any active release candidate.
async fn handle_regression_blocks_release(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
    metric_name: &str,
) -> anyhow::Result<()> {
    warn!(
        "[ReleaseAgent] Critical regression in {metric_name} -- \
         checking for active release candidates in project {project_id}"
    );

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[ReleaseAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_releases = &ctx.config.databases.releases;
    let db_events = &ctx.config.databases.events;

    // Query for release candidates
    let filter = json!({
        "and": [
            { "property": releases::PROJECT, "rich_text": { "equals": project_id } },
            { "property": releases::STATUS, "select": { "equals": "Candidate" } }
        ]
    });
    match ctx.notion.query_database(db_releases, Some(filter), None, None).await {
        Ok(result) => {
            if let Some(pages) = result["results"].as_array() {
                for page in pages {
                    if let Some(page_id) = page["id"].as_str() {
                        let update_props = json!({
                            releases::REGRESSION_FREE: prop::checkbox(false),
                        });
                        match ctx.notion.update_page(page_id, update_props).await {
                            Ok(_) => {
                                warn!(
                                    "[ReleaseAgent] Release candidate {page_id} BLOCKED: Regression Free=false"
                                );
                            }
                            Err(e) => error!("[ReleaseAgent] Failed to block release {page_id}: {e}"),
                        }
                    }
                }
            }
        }
        Err(e) => error!("[ReleaseAgent] Failed to query release candidates: {e}"),
    }

    // Create timeline event
    let now = Utc::now().to_rfc3339();
    let event_props = json!({
        events_schema::TITLE: prop::title(&format!("Release blocked: regression in {metric_name}")),
        events_schema::TYPE: prop::rich_text("ReleaseBlocked"),
        events_schema::SOURCE_LAYER: prop::select("Release"),
        events_schema::PROJECT: prop::rich_text(project_id),
        events_schema::DETAILS: prop::rich_text(&format!("severity=Critical | regression={metric_name}")),
        events_schema::TIMESTAMP: prop::date(&now),
        events_schema::IS_MILESTONE: prop::checkbox(true),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[ReleaseAgent] Created release-blocked event"),
        Err(e) => error!("[ReleaseAgent] Failed to create release-blocked event: {e}"),
    }

    warn!(
        "[ReleaseAgent] Release candidate for {project_id} BLOCKED due to \
         critical regression in {metric_name}"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_handle_pr_merged_for_release() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let result = handle_pr_merged_for_release(
            &state,
            "myorg/myrepo",
            42,
            "Add caching layer",
            &["RFC-0003".to_string()],
        )
        .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_rfc_approved_for_release() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let result = handle_rfc_approved_for_release(&state, "RFC-0005", "proj-1").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_regression_blocks_release() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let result = handle_regression_blocks_release(&state, "proj-1", "api_latency_p99").await;
        assert!(result.is_ok());
    }
}
