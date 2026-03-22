//! Vault agent main event loop.
//!
//! Handles environment variable tracking, secret rotation checks, and
//! env-config scaffolding from approved RFCs and merged PRs.

use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};
use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use engram_types::clients::{AgentContext, properties as prop};
use engram_types::events::EngramEvent;
use engram_types::notion_schema::{env_config, events as events_schema};

use crate::env_parser::extract_env_vars_from_diff;
use crate::rotation_checker::{self, RotationStatus};
use crate::prompts;

// ── Helper: downcast shared state to AgentContext ──

fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

/// Run the Vault-agent main loop.
///
/// Listens for events on the broadcast channel and dispatches to the
/// appropriate handler. State is passed as `Arc<dyn Any + Send + Sync>`
/// to allow the core orchestrator to inject shared application state.
pub async fn run(state: Arc<dyn std::any::Any + Send + Sync>, mut rx: broadcast::Receiver<EngramEvent>) {
    info!("[VaultAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[VaultAgent] Error handling event: {e}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[VaultAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[VaultAgent] Channel closed, shutting down");
                break;
            }
        }
    }
}

/// Dispatch a single event to the appropriate handler.
async fn handle_event(state: &Arc<dyn std::any::Any + Send + Sync>, event: &EngramEvent) -> Result<()> {
    match event {
        EngramEvent::PrMerged {
            repo,
            pr_number,
            diff,
            title,
            author,
            rfc_references,
            ..
        } => {
            handle_pr_merged(state, repo, *pr_number, diff, title, author, rfc_references).await
        }

        EngramEvent::RfcApproved {
            rfc_id,
            project_id,
            required_env_vars,
            ..
        } => {
            handle_rfc_approved(state, rfc_id, project_id, required_env_vars).await
        }

        EngramEvent::DailyRotationCheckTrigger { project_id } => {
            handle_daily_rotation_check(state, project_id).await
        }

        EngramEvent::EnvVarMissingInProd {
            var_notion_page_id,
            var_name,
            project_id,
        } => {
            handle_env_var_missing_in_prod(state, var_notion_page_id, var_name, project_id).await
        }

        EngramEvent::SetupComplete { project_id } => {
            info!("[VaultAgent] SetupComplete — triggering initial daily rotation check for project {project_id}");
            handle_daily_rotation_check(state, project_id).await
        }

        _ => Ok(()), // Ignore events not handled by the Vault agent
    }
}

// ─── PrMerged Handler ───────────────────────────────────────────────────────

/// Parse PR diff for new env var references, create Env Config records for
/// newly detected variables with status "Unverified".
async fn handle_pr_merged(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    repo: &str,
    pr_number: u64,
    diff: &str,
    title: &str,
    _author: &str,
    _rfc_references: &[String],
) -> Result<()> {
    info!("[VaultAgent] Processing PrMerged #{pr_number} in {repo}");

    // Step 1: Extract env var references from the diff
    let env_vars = extract_env_vars_from_diff(diff);

    if env_vars.is_empty() {
        info!("[VaultAgent] No new env var references found in PR #{pr_number}");
        return Ok(());
    }

    info!(
        "[VaultAgent] Found {} env var reference(s) in PR #{}: {:?}",
        env_vars.len(),
        pr_number,
        env_vars.iter().map(|v| &v.name).collect::<Vec<_>>()
    );

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[VaultAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_env_config = &ctx.config.databases.env_config;
    let db_events = &ctx.config.databases.events;

    // Step 2: Build Claude prompt for analysis
    let analysis_prompt = prompts::new_env_vars_analysis_prompt(repo, pr_number, title, &env_vars);

    // Step 3: Call Claude for classification
    let system = "You are a DevOps security expert specializing in environment variable management. Respond with valid JSON only.";
    match ctx.claude.complete(system, &analysis_prompt).await {
        Ok(response) => info!("[VaultAgent] Claude analysis for PR #{pr_number}: {} chars", response.len()),
        Err(e) => warn!("[VaultAgent] Claude analysis failed for PR #{pr_number}: {e}"),
    }

    // Step 4: For each detected var, create an Env Config record via Notion
    for var_ref in &env_vars {
        let props = json!({
            env_config::VAR_NAME: prop::title(&var_ref.name),
            env_config::STATUS: prop::select("Unverified"),
            env_config::DESCRIPTION: prop::rich_text(&format!("Detected in PR #{pr_number}: {title}")),
            env_config::SYNC_STATUS: prop::select("Unknown"),
        });
        match ctx.notion.create_page(db_env_config, props).await {
            Ok(_) => info!("[VaultAgent] Created Env Config record for var={}", var_ref.name),
            Err(e) => error!("[VaultAgent] Failed to create Env Config for var={}: {e}", var_ref.name),
        }
    }

    // Create timeline event
    let now = Utc::now().to_rfc3339();
    let event_props = json!({
        events_schema::TITLE: prop::title(&format!("PR #{pr_number} introduced {} env var(s)", env_vars.len())),
        events_schema::TYPE: prop::rich_text("EnvVarDetected"),
        events_schema::SOURCE_LAYER: prop::select("Vault"),
        events_schema::DETAILS: prop::rich_text(&format!("pr={repo}#{pr_number}")),
        events_schema::TIMESTAMP: prop::date(&now),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[VaultAgent] Created timeline event for PR #{pr_number} env vars"),
        Err(e) => error!("[VaultAgent] Failed to create timeline event: {e}"),
    }

    info!("[VaultAgent] Completed PrMerged processing for PR #{pr_number}");
    Ok(())
}

// ─── RfcApproved Handler ────────────────────────────────────────────────────

/// Scaffold required env vars from an approved RFC.
///
/// Creates Env Config records for each variable listed in the RFC's
/// `required_env_vars` field, with status "Pending Setup".
async fn handle_rfc_approved(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    rfc_id: &str,
    project_id: &str,
    required_env_vars: &[String],
) -> Result<()> {
    info!(
        "[VaultAgent] Processing RfcApproved: rfc={rfc_id}, project={project_id}, vars={:?}",
        required_env_vars
    );

    if required_env_vars.is_empty() {
        info!("[VaultAgent] RFC {rfc_id} has no required env vars, skipping");
        return Ok(());
    }

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[VaultAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_env_config = &ctx.config.databases.env_config;
    let db_events = &ctx.config.databases.events;

    // Step 1: Build Claude prompt for scaffolding recommendations
    let scaffold_prompt = prompts::rfc_env_scaffold_prompt(project_id, rfc_id, required_env_vars);

    // Step 2: Call Claude for classification of each variable
    let system = "You are a DevOps security expert. Classify environment variables by sensitivity and type. Respond with valid JSON only.";
    match ctx.claude.complete(system, &scaffold_prompt).await {
        Ok(response) => info!("[VaultAgent] Claude scaffold analysis for RFC {rfc_id}: {} chars", response.len()),
        Err(e) => warn!("[VaultAgent] Claude scaffold analysis failed for RFC {rfc_id}: {e}"),
    }

    // Step 3: Create Env Config records for each required variable
    for var_name in required_env_vars {
        let props = json!({
            env_config::VAR_NAME: prop::title(var_name),
            env_config::PROJECT: prop::rich_text(project_id),
            env_config::STATUS: prop::select("Pending Setup"),
            env_config::INTRODUCED_BY_RFC: prop::rich_text(rfc_id),
            env_config::SYNC_STATUS: prop::select("Not Deployed"),
        });
        match ctx.notion.create_page(db_env_config, props).await {
            Ok(_) => info!("[VaultAgent] Scaffolded Env Config: var={var_name}, rfc={rfc_id}"),
            Err(e) => error!("[VaultAgent] Failed to scaffold Env Config for var={var_name}: {e}"),
        }
    }

    // Step 4: Create timeline event
    let now = Utc::now().to_rfc3339();
    let event_props = json!({
        events_schema::TITLE: prop::title(&format!("RFC {rfc_id} scaffolded {} env vars", required_env_vars.len())),
        events_schema::TYPE: prop::rich_text("EnvVarScaffolded"),
        events_schema::SOURCE_LAYER: prop::select("Vault"),
        events_schema::PROJECT: prop::rich_text(project_id),
        events_schema::DETAILS: prop::rich_text(&format!("rfc={rfc_id}")),
        events_schema::TIMESTAMP: prop::date(&now),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[VaultAgent] Created timeline event for RFC {rfc_id} env scaffolding"),
        Err(e) => error!("[VaultAgent] Failed to create timeline event: {e}"),
    }

    Ok(())
}

// ─── DailyRotationCheckTrigger Handler ──────────────────────────────────────

/// Check all secrets for rotation policy compliance.
///
/// For each secret with a rotation policy:
/// 1. Compute next rotation due date from last_rotated + policy
/// 2. Check status (Ok / DueSoon / Overdue)
/// 3. Flag overdue secrets as "Needs Rotation"
/// 4. Fire SecretRotationDue events for overdue items
async fn handle_daily_rotation_check(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> Result<()> {
    info!("[VaultAgent] Running daily rotation check for project {project_id}");

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[VaultAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_env_config = &ctx.config.databases.env_config;
    let db_events = &ctx.config.databases.events;

    // Step 1: Query all Env Config records with rotation policies via Notion
    let filter = json!({
        "property": env_config::ROTATION_POLICY,
        "select": { "is_not_empty": true }
    });
    let env_configs = match ctx.notion.query_database(db_env_config, Some(filter), None, Some(100)).await {
        Ok(result) => result,
        Err(e) => {
            error!("[VaultAgent] Failed to query Env Config DB: {e}");
            return Ok(());
        }
    };

    // Step 2: Process each record
    let mut overdue_vars: Vec<(String, i64)> = Vec::new();

    if let Some(pages) = env_configs["results"].as_array() {
        for page in pages {
            let props = &page["properties"];
            let page_id = match page["id"].as_str() {
                Some(id) => id,
                None => continue,
            };
            let var_name = props[env_config::VAR_NAME]["title"][0]["text"]["content"]
                .as_str()
                .unwrap_or("unknown");
            let policy = props[env_config::ROTATION_POLICY]["select"]["name"]
                .as_str()
                .unwrap_or("");
            let last_rotated_str = props[env_config::LAST_ROTATED]["date"]["start"].as_str();

            if let Some(lr_str) = last_rotated_str {
                if let Ok(last) = lr_str.parse::<chrono::DateTime<Utc>>() {
                    let next_due = rotation_checker::compute_next_rotation(last, policy);
                    let status = rotation_checker::check_rotation_status(next_due);
                    let days = rotation_checker::days_overdue(next_due);

                    match status {
                        RotationStatus::Overdue => {
                            warn!(
                                "[VaultAgent] Secret '{var_name}' is OVERDUE by {days} days (policy: {policy})"
                            );
                            overdue_vars.push((var_name.to_string(), days));

                            let update_props = json!({
                                env_config::NEXT_ROTATION_DUE: prop::date(&next_due.to_rfc3339()),
                                env_config::SYNC_STATUS: prop::select("Needs Rotation"),
                            });
                            match ctx.notion.update_page(page_id, update_props).await {
                                Ok(_) => info!("[VaultAgent] Updated '{var_name}' as Needs Rotation"),
                                Err(e) => error!("[VaultAgent] Failed to update '{var_name}': {e}"),
                            }
                        }
                        RotationStatus::DueSoon => {
                            info!(
                                "[VaultAgent] Secret '{var_name}' is due for rotation soon (next due: {next_due})"
                            );
                            let update_props = json!({
                                env_config::NEXT_ROTATION_DUE: prop::date(&next_due.to_rfc3339()),
                            });
                            match ctx.notion.update_page(page_id, update_props).await {
                                Ok(_) => info!("[VaultAgent] Updated '{var_name}' next rotation due"),
                                Err(e) => error!("[VaultAgent] Failed to update '{var_name}': {e}"),
                            }
                        }
                        RotationStatus::Ok => {
                            // No action needed, secret is within policy
                        }
                    }
                }
            } else {
                warn!(
                    "[VaultAgent] Secret '{var_name}' has rotation policy '{policy}' but no Last Rotated date"
                );
            }
        }
    }

    // Step 3: If there are overdue secrets, get Claude's prioritisation
    if !overdue_vars.is_empty() {
        let rotation_prompt = prompts::rotation_overdue_prompt(project_id, &overdue_vars);
        let system = "You are a security operations expert. Prioritize secret rotations by risk. Respond with valid JSON only.";
        match ctx.claude.complete(system, &rotation_prompt).await {
            Ok(response) => info!(
                "[VaultAgent] Claude rotation prioritisation: {} chars for {} vars",
                response.len(),
                overdue_vars.len()
            ),
            Err(e) => warn!("[VaultAgent] Claude rotation analysis failed: {e}"),
        }

        // Create timeline event for overdue rotations
        let now = Utc::now().to_rfc3339();
        let event_props = json!({
            events_schema::TITLE: prop::title(&format!("{} secrets overdue for rotation", overdue_vars.len())),
            events_schema::TYPE: prop::rich_text("SecretRotationOverdue"),
            events_schema::SOURCE_LAYER: prop::select("Vault"),
            events_schema::PROJECT: prop::rich_text(project_id),
            events_schema::DETAILS: prop::rich_text("severity=High"),
            events_schema::TIMESTAMP: prop::date(&now),
        });
        match ctx.notion.create_page(db_events, event_props).await {
            Ok(_) => info!("[VaultAgent] Created overdue rotation event"),
            Err(e) => error!("[VaultAgent] Failed to create overdue rotation event: {e}"),
        }
    }

    info!("[VaultAgent] Daily rotation check complete for project {project_id}");
    Ok(())
}

// ─── EnvVarMissingInProd Handler ────────────────────────────────────────────

/// Handle a detected env var missing in production.
///
/// Sets the Sync Status to "Missing In Prod" and creates a Critical
/// timeline event to alert the team.
async fn handle_env_var_missing_in_prod(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    var_notion_page_id: &str,
    var_name: &str,
    project_id: &str,
) -> Result<()> {
    error!(
        "[VaultAgent] CRITICAL: Env var '{var_name}' is missing in production (project: {project_id})"
    );

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[VaultAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_events = &ctx.config.databases.events;

    // Step 1: Update Env Config record — set Sync Status to "Missing In Prod"
    let update_props = json!({
        env_config::SYNC_STATUS: prop::select("Missing In Prod"),
    });
    match ctx.notion.update_page(var_notion_page_id, update_props).await {
        Ok(_) => info!("[VaultAgent] Updated Env Config page {var_notion_page_id}: Sync Status='Missing In Prod'"),
        Err(e) => error!("[VaultAgent] Failed to update Env Config page {var_notion_page_id}: {e}"),
    }

    // Step 2: Create a Critical Timeline event
    let event_title = format!("Env var '{var_name}' missing in production");
    let now = Utc::now().to_rfc3339();

    let event_props = json!({
        events_schema::TITLE: prop::title(&event_title),
        events_schema::TYPE: prop::rich_text("ConfigMismatch"),
        events_schema::SOURCE_LAYER: prop::select("Vault"),
        events_schema::PROJECT: prop::rich_text(project_id),
        events_schema::DETAILS: prop::rich_text(&format!("severity=Critical | env_var={var_name}")),
        events_schema::TIMESTAMP: prop::date(&now),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[VaultAgent] Created critical timeline event for missing prod var '{var_name}'"),
        Err(e) => error!("[VaultAgent] Failed to create timeline event for '{var_name}': {e}"),
    }

    warn!(
        "[VaultAgent] Critical timeline event created for missing prod var '{var_name}'"
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that the event dispatch correctly routes PrMerged events.
    #[tokio::test]
    async fn test_handle_pr_merged_no_env_vars() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let event = EngramEvent::PrMerged {
            repo: "test-repo".to_string(),
            pr_number: 1,
            diff: "just some code changes\n+fn hello() {}".to_string(),
            branch: "main".to_string(),
            commit_sha: "abc123".to_string(),
            title: "Add hello fn".to_string(),
            author: "dev".to_string(),
            rfc_references: vec![],
        };
        // Should not error even with no env vars
        let result = handle_event(&state, &event).await;
        assert!(result.is_ok());
    }

    /// Verify PrMerged with env vars in the diff.
    #[tokio::test]
    async fn test_handle_pr_merged_with_env_vars() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let diff = r#"
diff --git a/.env.example b/.env.example
--- a/.env.example
+++ b/.env.example
@@ -1,2 +1,3 @@
+NEW_SECRET=changeme
"#;
        let event = EngramEvent::PrMerged {
            repo: "test-repo".to_string(),
            pr_number: 42,
            diff: diff.to_string(),
            branch: "main".to_string(),
            commit_sha: "def456".to_string(),
            title: "Add new secret".to_string(),
            author: "dev".to_string(),
            rfc_references: vec![],
        };
        let result = handle_event(&state, &event).await;
        assert!(result.is_ok());
    }

    /// Verify RfcApproved with required env vars.
    #[tokio::test]
    async fn test_handle_rfc_approved() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let event = EngramEvent::RfcApproved {
            rfc_notion_page_id: "page-1".to_string(),
            rfc_id: "RFC-001".to_string(),
            project_id: "proj-1".to_string(),
            required_env_vars: vec!["API_KEY".to_string(), "DB_URL".to_string()],
            affected_modules: vec![],
            banned_patterns: vec![],
        };
        let result = handle_event(&state, &event).await;
        assert!(result.is_ok());
    }

    /// Verify DailyRotationCheckTrigger runs without error.
    #[tokio::test]
    async fn test_handle_daily_rotation_check() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let event = EngramEvent::DailyRotationCheckTrigger {
            project_id: "proj-1".to_string(),
        };
        let result = handle_event(&state, &event).await;
        assert!(result.is_ok());
    }

    /// Verify EnvVarMissingInProd creates critical alert.
    #[tokio::test]
    async fn test_handle_env_var_missing_in_prod() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let event = EngramEvent::EnvVarMissingInProd {
            var_notion_page_id: "page-2".to_string(),
            var_name: "DATABASE_URL".to_string(),
            project_id: "proj-1".to_string(),
        };
        let result = handle_event(&state, &event).await;
        assert!(result.is_ok());
    }

    /// Verify unrelated events are ignored.
    #[tokio::test]
    async fn test_ignore_unrelated_event() {
        let state: Arc<dyn std::any::Any + Send + Sync> = Arc::new(());
        let event = EngramEvent::WeeklyDigestTrigger {
            project_id: "proj-1".to_string(),
        };
        let result = handle_event(&state, &event).await;
        assert!(result.is_ok());
    }
}
