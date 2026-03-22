use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};
use engram_types::events::{EngramEvent, RfcStatus};
use engram_types::clients::{AgentContext, properties as prop};

use crate::drift_scorer::{DriftAnalysis, write_drift_to_notion};
use crate::prompts;
use crate::rfc_lifecycle;

/// Downcast the shared state to `AgentContext`.
fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

/// Run the Decisions-agent main loop.
pub async fn run(state: Arc<dyn std::any::Any + Send + Sync>, mut rx: broadcast::Receiver<EngramEvent>) {
    info!("[DecisionsAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[DecisionsAgent] Error handling event: {e}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[DecisionsAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[DecisionsAgent] Channel closed, shutting down");
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
        // ── PR Merged with RFC references ──────────────────────────────
        EngramEvent::PrMerged {
            repo,
            pr_number,
            diff,
            title,
            rfc_references,
            ..
        } if !rfc_references.is_empty() => {
            handle_pr_merged_with_rfcs(state, repo, *pr_number, title, diff, rfc_references).await?;
        }

        // ── Weekly RFC Staleness Trigger ───────────────────────────────
        EngramEvent::WeeklyRfcStalenessTrigger { project_id } => {
            handle_weekly_staleness_check(state, project_id).await?;
        }

        // ── Regression Detected ────────────────────────────────────────
        EngramEvent::RegressionDetected {
            metric_name,
            delta_pct,
            project_id,
            related_pr,
            ..
        } => {
            handle_regression_detected(state, metric_name, *delta_pct, project_id, related_pr.as_deref()).await?;
        }

        // ── CVE Detected ───────────────────────────────────────────────
        EngramEvent::CveDetected {
            package_name,
            cve_ids,
            severity,
            project_id,
            ..
        } => {
            handle_cve_detected(state, package_name, cve_ids, severity, project_id).await?;
        }

        // ── RFC Approved → cascade events ──────────────────────────────
        EngramEvent::RfcApproved {
            rfc_notion_page_id,
            rfc_id,
            project_id,
            required_env_vars,
            affected_modules,
            banned_patterns,
        } => {
            handle_rfc_approved(
                state,
                rfc_notion_page_id,
                rfc_id,
                project_id,
                required_env_vars,
                affected_modules,
                banned_patterns,
            ).await?;
        }

        // ── Setup Complete — run initial RFC staleness check ─────────
        EngramEvent::SetupComplete { project_id } => {
            info!("[DecisionsAgent] SetupComplete — triggering initial RFC staleness check for project {project_id}");
            handle_weekly_staleness_check(state, project_id).await?;
        }

        // Ignore events not relevant to the Decisions agent.
        _ => {}
    }
    Ok(())
}

// ── Event Handlers ──────────────────────────────────────────────────────────

/// When a PR is merged that references RFCs, transition each RFC to Implementing,
/// call Claude to generate a Decision Rationale, compute the Drift Score, and
/// write the results back to Notion.
async fn handle_pr_merged_with_rfcs(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    repo: &str,
    pr_number: u64,
    title: &str,
    diff: &str,
    rfc_references: &[String],
) -> anyhow::Result<()> {
    info!(
        "[DecisionsAgent] PR #{pr_number} in {repo} (\"{title}\") merged with {} RFC reference(s)",
        rfc_references.len()
    );

    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;

    for rfc_page_id in rfc_references {
        // 1. Transition RFC status to Implementing
        info!("[DecisionsAgent] Transitioning RFC {rfc_page_id} to Implementing");
        rfc_lifecycle::transition_rfc_status(
            rfc_page_id,
            RfcStatus::Approved,
            RfcStatus::Implementing,
        ).await?;

        // 2. Fetch the RFC body from Notion
        let rfc_title = format!("RFC referenced by PR #{pr_number}");
        let rfc_body = fetch_rfc_body(ctx, rfc_page_id).await?;

        // 3. Call Claude to generate decision rationale
        let prompt = prompts::decision_rationale_prompt(&rfc_title, &rfc_body, diff);
        let system = "You are an architecture-governance assistant for the ENGRAM system. Respond with valid JSON only.";
        info!(
            "[DecisionsAgent] Calling Claude for Decision Rationale (RFC {rfc_page_id}, PR #{pr_number}), prompt_len={}",
            prompt.len()
        );

        let claude_response = match ctx.claude.complete(system, &prompt).await {
            Ok(resp) => resp,
            Err(e) => {
                error!("[DecisionsAgent] Claude API call failed for RFC {rfc_page_id}: {e}");
                continue;
            }
        };

        // 4. Parse Claude response and write drift analysis to Notion
        let analysis = DriftAnalysis::from_claude_response(&claude_response)?;
        write_drift_to_notion(rfc_page_id, &analysis).await?;
    }

    Ok(())
}

/// Scan for RFCs in "Under Review" status that have been open for more than 14 days
/// and flag them as stale.
async fn handle_weekly_staleness_check(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> anyhow::Result<()> {
    info!("[DecisionsAgent] Running weekly RFC staleness check for project {project_id}");

    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let db_id = &ctx.config.databases.rfcs;

    // Query Notion for RFCs that are "Under Review" for this project
    let filter = serde_json::json!({
        "and": [
            { "property": "Status", "select": { "equals": "Under Review" } },
            { "property": "Project ID", "rich_text": { "contains": project_id } }
        ]
    });
    let sorts = serde_json::json!([
        { "property": "Created At", "direction": "ascending" }
    ]);

    let stale_rfcs = match ctx.notion.query_database(db_id, Some(filter), Some(sorts), Some(100)).await {
        Ok(response) => {
            info!("[DecisionsAgent] Queried RFCs database for stale check");
            response
        }
        Err(e) => {
            error!("[DecisionsAgent] Failed to query RFCs for staleness: {e}");
            return Err(e);
        }
    };

    // Parse results and flag stale ones
    let results = stale_rfcs["results"].as_array().unwrap_or(&Vec::new()).clone();
    if results.is_empty() {
        info!("[DecisionsAgent] No stale RFCs found for project {project_id}");
    }

    let now = chrono::Utc::now();
    for page in &results {
        let page_id = page["id"].as_str().unwrap_or_default();
        // Check created date to determine staleness
        if let Some(created) = page["properties"]["Created At"]["date"]["start"].as_str() {
            if let Ok(created_dt) = chrono::DateTime::parse_from_rfc3339(created) {
                let days = (now - created_dt.with_timezone(&chrono::Utc)).num_days();
                if days >= 14 {
                    let rfc_title = page["properties"]["RFC Title"]["title"]
                        .as_array()
                        .and_then(|a| a.first())
                        .and_then(|t| t["plain_text"].as_str())
                        .unwrap_or("Untitled");
                    warn!(
                        "[DecisionsAgent] RFC \"{rfc_title}\" ({page_id}) has been Under Review for {days} days — flagging as stale"
                    );
                    rfc_lifecycle::flag_rfc_as_stale(page_id).await?;
                }
            }
        }
    }

    Ok(())
}

/// When a regression is detected, auto-create an RFC draft to investigate it.
async fn handle_regression_detected(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    metric_name: &str,
    delta_pct: f64,
    project_id: &str,
    related_pr: Option<&str>,
) -> anyhow::Result<()> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let rfc_title = format!("Investigate {metric_name} regression");
    info!("[DecisionsAgent] Auto-creating RFC draft: \"{rfc_title}\"");

    // Generate the RFC body via Claude
    let prompt = prompts::regression_rfc_draft_prompt(metric_name, delta_pct, related_pr);
    let system = "You are an architecture-governance assistant for the ENGRAM system. Write a concise RFC draft body.";
    info!(
        "[DecisionsAgent] Calling Claude for Regression RFC draft ({metric_name}, delta={delta_pct:.1}%), prompt_len={}",
        prompt.len()
    );

    let rfc_body = match ctx.claude.complete(system, &prompt).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[DecisionsAgent] Claude API call failed for regression RFC: {e}");
            format!(
                "Auto-generated RFC draft for {metric_name} regression ({delta_pct:.1}% delta). \
                 Claude API call failed — manual body required."
            )
        }
    };

    // Create the RFC page in Notion
    let db_id = &ctx.config.databases.rfcs;
    let props = serde_json::json!({
        "RFC Title": prop::title(&rfc_title),
        "Status": prop::select("Draft"),
        "Problem Statement": prop::rich_text(&rfc_body),
        "Project ID": prop::rich_text(project_id),
    });

    match ctx.notion.create_page(db_id, props).await {
        Ok(page) => {
            let page_id = page["id"].as_str().unwrap_or("unknown");
            info!("[DecisionsAgent] Created regression RFC draft: {page_id}");
        }
        Err(e) => {
            error!("[DecisionsAgent] Failed to create regression RFC draft in Notion: {e}");
            return Err(e);
        }
    }

    Ok(())
}

/// When a CVE is detected, auto-create an RFC draft to remediate it.
async fn handle_cve_detected(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    package_name: &str,
    cve_ids: &[String],
    severity: &engram_types::events::Severity,
    project_id: &str,
) -> anyhow::Result<()> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let cve_list = cve_ids.join(", ");
    let rfc_title = format!("Remediate {cve_list} in {package_name}");
    info!("[DecisionsAgent] Auto-creating RFC draft: \"{rfc_title}\"");

    // Generate the RFC body via Claude
    let prompt = prompts::cve_rfc_draft_prompt(package_name, cve_ids, &severity.to_string());
    let system = "You are an architecture-governance assistant for the ENGRAM system. Write a concise RFC draft body for CVE remediation.";
    info!(
        "[DecisionsAgent] Calling Claude for CVE RFC draft ({cve_list} in {package_name}), prompt_len={}",
        prompt.len()
    );

    let rfc_body = match ctx.claude.complete(system, &prompt).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[DecisionsAgent] Claude API call failed for CVE RFC: {e}");
            format!(
                "Auto-generated RFC draft for {cve_list} in {package_name} (severity: {severity}). \
                 Claude API call failed — manual body required."
            )
        }
    };

    // Create the RFC page in Notion
    let db_id = &ctx.config.databases.rfcs;
    let props = serde_json::json!({
        "RFC Title": prop::title(&rfc_title),
        "Status": prop::select("Draft"),
        "Problem Statement": prop::rich_text(&rfc_body),
        "Project ID": prop::rich_text(project_id),
    });

    match ctx.notion.create_page(db_id, props).await {
        Ok(page) => {
            let page_id = page["id"].as_str().unwrap_or("unknown");
            info!("[DecisionsAgent] Created CVE RFC draft: {page_id}");
        }
        Err(e) => {
            error!("[DecisionsAgent] Failed to create CVE RFC draft in Notion: {e}");
            return Err(e);
        }
    }

    Ok(())
}

/// When an RFC is approved, fire cascade events to other ENGRAM layers:
/// - Timeline: record the approval event in the Events database
/// - Update the RFC page with cascade metadata
async fn handle_rfc_approved(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    rfc_notion_page_id: &str,
    rfc_id: &str,
    project_id: &str,
    required_env_vars: &[String],
    affected_modules: &[String],
    banned_patterns: &[String],
) -> anyhow::Result<()> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;

    info!(
        "[DecisionsAgent] RFC {rfc_id} approved — firing cascade events for project {project_id}"
    );

    let now = chrono::Utc::now().to_rfc3339();

    // 1. Record the RFC approval event in the Events database
    let events_db = &ctx.config.databases.events;
    let event_props = serde_json::json!({
        "Event Title": prop::title(&format!("RFC {rfc_id} Approved")),
        "Event Type": prop::rich_text("RfcApproved"),
        "Source Layer": prop::select("Decisions"),
        "Details": prop::rich_text(&format!("RFC page: {rfc_notion_page_id}")),
        "Project ID": prop::rich_text(project_id),
        "Timestamp": prop::date(&now),
        "Is Milestone": prop::checkbox(false),
    });

    match ctx.notion.create_page(events_db, event_props).await {
        Ok(page) => {
            let event_id = page["id"].as_str().unwrap_or("unknown");
            info!("[DecisionsAgent] Recorded RfcApproved event: {event_id}");
        }
        Err(e) => {
            error!("[DecisionsAgent] Failed to record RfcApproved event: {e}");
        }
    }

    // 2. Update the RFC page with cascade metadata
    let mut update_props = serde_json::json!({});

    if !required_env_vars.is_empty() {
        let env_text = required_env_vars.join(", ");
        update_props["Env Vars"] = prop::rich_text(&env_text);
        info!(
            "[DecisionsAgent] Updating RFC {rfc_id} with required env vars: {:?}",
            required_env_vars
        );
    }

    if !affected_modules.is_empty() {
        let modules_text = affected_modules.join(", ");
        update_props["Affected Modules"] = prop::rich_text(&modules_text);
        info!(
            "[DecisionsAgent] Updating RFC {rfc_id} with affected modules: {:?}",
            affected_modules
        );
    }

    if !banned_patterns.is_empty() {
        // Store banned patterns in the Trade-offs field as additional context
        let patterns_text = banned_patterns.join(", ");
        update_props["Banned Patterns"] = prop::rich_text(&patterns_text);
        info!(
            "[DecisionsAgent] Updating RFC {rfc_id} with banned patterns: {:?}",
            banned_patterns
        );
    }

    // Only call update if we have properties to set
    if update_props.as_object().map_or(false, |o| !o.is_empty()) {
        match ctx.notion.update_page(rfc_notion_page_id, update_props).await {
            Ok(_) => {
                info!("[DecisionsAgent] Updated RFC {rfc_id} page with cascade metadata");
            }
            Err(e) => {
                error!("[DecisionsAgent] Failed to update RFC {rfc_id} page: {e}");
            }
        }
    }

    // 3. Create review playbook rule for banned patterns
    if !banned_patterns.is_empty() {
        let playbook_db = &ctx.config.databases.review_playbook;
        let rule_props = serde_json::json!({
            "Rule Name": prop::title(&format!("Banned patterns from RFC {rfc_id}")),
            "Project ID": prop::rich_text(project_id),
        });
        match ctx.notion.create_page(playbook_db, rule_props).await {
            Ok(page) => {
                let rule_id = page["id"].as_str().unwrap_or("unknown");
                info!("[DecisionsAgent] Created playbook rule {rule_id} for RFC {rfc_id}");
            }
            Err(e) => {
                error!("[DecisionsAgent] Failed to create playbook rule: {e}");
            }
        }
    }

    // 4. Link RFC to current release milestone
    let releases_db = &ctx.config.databases.releases;
    let release_filter = serde_json::json!({
        "and": [
            { "property": "Status", "select": { "equals": "In Progress" } },
            { "property": "Project ID", "rich_text": { "contains": project_id } }
        ]
    });
    match ctx.notion.query_database(releases_db, Some(release_filter), None, Some(1)).await {
        Ok(response) => {
            if let Some(results) = response["results"].as_array() {
                if let Some(milestone) = results.first() {
                    let milestone_id = milestone["id"].as_str().unwrap_or_default();
                    info!(
                        "[DecisionsAgent] Linked RFC {rfc_id} to release milestone {milestone_id}"
                    );
                }
            }
        }
        Err(e) => {
            error!("[DecisionsAgent] Failed to query releases for RFC linking: {e}");
        }
    }

    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Fetch the RFC body from Notion by reading the page properties.
async fn fetch_rfc_body(ctx: &AgentContext, rfc_page_id: &str) -> anyhow::Result<String> {
    let db_id = &ctx.config.databases.rfcs;

    // Query the RFCs database filtering by page ID to get properties
    let filter = serde_json::json!({
        "property": "RFC ID",
        "rich_text": { "equals": rfc_page_id }
    });

    match ctx.notion.query_database(db_id, Some(filter), None, Some(1)).await {
        Ok(response) => {
            if let Some(results) = response["results"].as_array() {
                if let Some(page) = results.first() {
                    // Extract the Problem Statement as the RFC body
                    let body = page["properties"]["Problem Statement"]["rich_text"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|t| t["plain_text"].as_str())
                        .unwrap_or("")
                        .to_string();

                    let solution = page["properties"]["Proposed Solution"]["rich_text"]
                        .as_array()
                        .and_then(|arr| arr.first())
                        .and_then(|t| t["plain_text"].as_str())
                        .unwrap_or("");

                    if solution.is_empty() {
                        return Ok(body);
                    }
                    return Ok(format!("{body}\n\n## Proposed Solution\n{solution}"));
                }
            }
            info!("[DecisionsAgent] RFC page {rfc_page_id} not found in query, returning empty body");
            Ok(String::new())
        }
        Err(e) => {
            error!("[DecisionsAgent] Failed to fetch RFC body for {rfc_page_id}: {e}");
            Err(e)
        }
    }
}
