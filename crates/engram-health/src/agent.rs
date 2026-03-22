//! Health Agent — handles `WeeklyDigestTrigger` events.
//!
//! Queries all six layer databases, computes per-layer health scores,
//! calls Claude for AI Narrative + Key Risks + Key Wins, and writes
//! Health Report + Engineering Digest records to Notion.

use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};

use serde::Deserialize;
use serde_json::json;

use engram_types::clients::{AgentContext, properties as prop};
use engram_types::events::EngramEvent;
use engram_types::notion_schema::{health_reports as hr_schema, engineering_digest as ed_schema};

use crate::digest_generator::{DigestData, generate_digest_prompt};
use crate::prompts;
use crate::score_computer;

/// Downcast the shared `Arc<dyn Any>` state to [`AgentContext`].
fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

/// Run the Health-agent main loop
pub async fn run(state: Arc<dyn std::any::Any + Send + Sync>, mut rx: broadcast::Receiver<EngramEvent>) {
    info!("[HealthAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[HealthAgent] Error handling event: {e}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[HealthAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[HealthAgent] Channel closed, shutting down");
                break;
            }
        }
    }
}

async fn handle_event(state: &Arc<dyn std::any::Any + Send + Sync>, event: &EngramEvent) -> anyhow::Result<()> {
    match event {
        EngramEvent::WeeklyDigestTrigger { project_id } => {
            handle_weekly_digest(state, project_id).await?;
        }
        EngramEvent::SetupComplete { project_id } => {
            info!("[HealthAgent] SetupComplete — triggering initial weekly digest for project {project_id}");
            handle_weekly_digest(state, project_id).await?;
        }
        _ => {} // Ignore unrelated events
    }
    Ok(())
}

/// Main handler for WeeklyDigestTrigger.
///
/// Steps:
/// 1. Query all six layer DBs to gather raw counts
/// 2. Compute per-layer health scores
/// 3. Compute overall score
/// 4. Build DigestData with weekly stats
/// 5. Call Claude for AI narrative, key risks, key wins
/// 6. Write Health Report record to Notion
/// 7. Write Engineering Digest record to Notion
async fn handle_weekly_digest(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> anyhow::Result<()> {
    info!("[HealthAgent] Processing WeeklyDigestTrigger for project {project_id}");

    // ── Step 1: Query layer databases ──

    info!("[HealthAgent] Querying Decisions layer (RFCs DB) for stale RFC counts");
    let (open_stale_rfcs, total_rfcs) = query_decisions_stats(state, project_id).await?;

    info!("[HealthAgent] Querying Pulse layer (Benchmarks DB) for benchmark statuses");
    let (normal_benchmarks, total_benchmarks) = query_pulse_stats(state, project_id).await?;

    info!("[HealthAgent] Querying Shield layer (Dependencies DB) for triage counts");
    let shield_stats = query_shield_stats(state, project_id).await?;

    info!("[HealthAgent] Querying Atlas layer (Modules + Knowledge Gaps DBs)");
    let (documented_modules, total_modules, open_gaps) = query_atlas_stats(state, project_id).await?;

    info!("[HealthAgent] Querying Vault layer (Env Config DB)");
    let (valid_secrets, total_secrets, missing_in_prod) = query_vault_stats(state, project_id).await?;

    info!("[HealthAgent] Querying Review layer (PR Reviews + Review Patterns DBs)");
    let (reviewed_prs, merged_prs, critical_patterns) = query_review_stats(state, project_id).await?;

    // ── Step 2: Compute per-layer health scores ──
    let decisions_health = score_computer::compute_decisions_health(open_stale_rfcs, total_rfcs);
    let pulse_health = score_computer::compute_pulse_health(normal_benchmarks, total_benchmarks);
    let shield_health = score_computer::compute_shield_health(
        shield_stats.triaged_critical, shield_stats.total_critical,
        shield_stats.triaged_high, shield_stats.total_high,
        shield_stats.triaged_medium, shield_stats.total_medium,
        shield_stats.triaged_low, shield_stats.total_low,
    );
    let atlas_health = score_computer::compute_atlas_health(documented_modules, total_modules, open_gaps);
    let vault_health = score_computer::compute_vault_health(valid_secrets, total_secrets, missing_in_prod);
    let review_health = score_computer::compute_review_health(reviewed_prs, merged_prs, critical_patterns);

    // ── Step 3: Compute overall score ──
    let overall_score = score_computer::compute_overall(
        decisions_health,
        pulse_health,
        shield_health,
        atlas_health,
        vault_health,
        review_health,
    );

    info!(
        "[HealthAgent] Scores — D:{decisions_health:.1} Pu:{pulse_health:.1} S:{shield_health:.1} \
         A:{atlas_health:.1} V:{vault_health:.1} R:{review_health:.1} Overall:{overall_score:.1}"
    );

    // ── Step 4: Build digest data ──
    let digest_data = build_digest_data(
        state,
        project_id,
        overall_score,
    ).await?;

    let period = chrono::Utc::now().format("%Y-W%V").to_string();
    let digest_summary = digest_data.to_summary();

    // ── Step 5: Call Claude for AI narrative ──
    info!("[HealthAgent] Calling Claude for health narrative");
    let narrative_prompt = prompts::health_narrative_prompt(
        project_id,
        &period,
        decisions_health,
        pulse_health,
        shield_health,
        atlas_health,
        vault_health,
        review_health,
        overall_score,
        digest_data.health_delta,
        &digest_summary,
    );

    let ai_narrative = call_claude_for_narrative(state, &narrative_prompt).await?;
    info!("[HealthAgent] Claude narrative received ({} chars)", ai_narrative.narrative.len());

    // ── Step 6: Write Health Report to Notion ──
    info!("[HealthAgent] Writing Health Report record to Notion");
    write_health_report(
        state,
        project_id,
        &period,
        decisions_health,
        pulse_health,
        shield_health,
        atlas_health,
        vault_health,
        review_health,
        overall_score,
        digest_data.health_delta,
        &ai_narrative,
    ).await?;

    // ── Step 7: Write Engineering Digest to Notion ──
    info!("[HealthAgent] Writing Engineering Digest record to Notion");
    write_engineering_digest(
        state,
        project_id,
        &digest_data,
        &ai_narrative.narrative,
    ).await?;

    // Generate digest prompt for logging/debugging
    let _digest_prompt = generate_digest_prompt(&digest_data);
    info!("[HealthAgent] Weekly digest completed for project {project_id}");

    Ok(())
}

// ─── Data structures ──────────────────────────────────────────────────────────

/// Shield layer vulnerability stats broken down by severity.
struct ShieldStats {
    triaged_critical: u32,
    total_critical: u32,
    triaged_high: u32,
    total_high: u32,
    triaged_medium: u32,
    total_medium: u32,
    triaged_low: u32,
    total_low: u32,
}

/// AI-generated health narrative response.
#[derive(Debug, Deserialize)]
struct HealthNarrative {
    #[serde(alias = "ai_narrative")]
    narrative: String,
    key_risks: Vec<String>,
    key_wins: Vec<String>,
}

// ─── Notion query result helpers ──────────────────────────────────────────────

/// Count the total number of results returned by a Notion query_database call.
fn count_results(response: &serde_json::Value) -> u32 {
    response["results"].as_array().map_or(0, |a| a.len() as u32)
}

/// Build a project filter that matches a "Project ID" rich_text property.
fn project_filter(project_id: &str) -> serde_json::Value {
    json!({
        "property": "Project ID",
        "rich_text": { "equals": project_id }
    })
}

// ─── Layer query helpers ──────────────────────────────────────────────────────

async fn query_decisions_stats(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> anyhow::Result<(u32, u32)> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let db_id = &ctx.config.databases.rfcs;

    // Total RFCs for this project
    let total_filter = project_filter(project_id);
    let total_resp = match ctx.notion.query_database(db_id, Some(total_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query RFCs DB: {e}");
            return Ok((0, 0));
        }
    };
    let total_rfcs = count_results(&total_resp);

    // Stale RFCs: status "Draft" or "Under Review" (simplified — age filtering
    // would require iterating results and checking dates client-side)
    let stale_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "or": [
                { "property": "Status", "select": { "equals": "Draft" } },
                { "property": "Status", "select": { "equals": "Under Review" } }
            ]}
        ]
    });
    let stale_resp = match ctx.notion.query_database(db_id, Some(stale_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query stale RFCs: {e}");
            return Ok((0, total_rfcs));
        }
    };
    let open_stale_rfcs = count_results(&stale_resp);

    info!("[HealthAgent] RFCs: {open_stale_rfcs} stale / {total_rfcs} total");
    Ok((open_stale_rfcs, total_rfcs))
}

async fn query_pulse_stats(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> anyhow::Result<(u32, u32)> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let db_id = &ctx.config.databases.benchmarks;

    // Total benchmarks
    let total_filter = project_filter(project_id);
    let total_resp = match ctx.notion.query_database(db_id, Some(total_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query Benchmarks DB: {e}");
            return Ok((0, 0));
        }
    };
    let total_benchmarks = count_results(&total_resp);

    // Normal benchmarks
    let normal_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Status", "select": { "equals": "Normal" } }
        ]
    });
    let normal_resp = match ctx.notion.query_database(db_id, Some(normal_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query normal Benchmarks: {e}");
            return Ok((0, total_benchmarks));
        }
    };
    let normal_benchmarks = count_results(&normal_resp);

    info!("[HealthAgent] Benchmarks: {normal_benchmarks} normal / {total_benchmarks} total");
    Ok((normal_benchmarks, total_benchmarks))
}

async fn query_shield_stats(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> anyhow::Result<ShieldStats> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let db_id = &ctx.config.databases.dependencies;

    let triaged_statuses = ["Accepted Risk", "Triaged", "Resolved"];
    let severities = ["Critical", "High", "Medium", "Low"];

    let mut stats = ShieldStats {
        triaged_critical: 0, total_critical: 0,
        triaged_high: 0, total_high: 0,
        triaged_medium: 0, total_medium: 0,
        triaged_low: 0, total_low: 0,
    };

    for severity in &severities {
        // Total for this severity
        let sev_filter = json!({
            "and": [
                { "property": "Project ID", "rich_text": { "equals": project_id } },
                { "property": "Severity", "select": { "equals": severity } }
            ]
        });
        let total = match ctx.notion.query_database(db_id, Some(sev_filter), None, None).await {
            Ok(resp) => count_results(&resp),
            Err(e) => {
                error!("[HealthAgent] Failed to query Dependencies for {severity}: {e}");
                0
            }
        };

        // Triaged for this severity
        let triage_conditions: Vec<serde_json::Value> = triaged_statuses.iter()
            .map(|s| json!({ "property": "Triage Status", "select": { "equals": s } }))
            .collect();
        let triaged_filter = json!({
            "and": [
                { "property": "Project ID", "rich_text": { "equals": project_id } },
                { "property": "Severity", "select": { "equals": severity } },
                { "or": triage_conditions }
            ]
        });
        let triaged = match ctx.notion.query_database(db_id, Some(triaged_filter), None, None).await {
            Ok(resp) => count_results(&resp),
            Err(e) => {
                error!("[HealthAgent] Failed to query triaged Dependencies for {severity}: {e}");
                0
            }
        };

        match *severity {
            "Critical" => { stats.triaged_critical = triaged; stats.total_critical = total; }
            "High"     => { stats.triaged_high = triaged;     stats.total_high = total; }
            "Medium"   => { stats.triaged_medium = triaged;   stats.total_medium = total; }
            "Low"      => { stats.triaged_low = triaged;      stats.total_low = total; }
            _ => {}
        }
    }

    info!(
        "[HealthAgent] Shield: C={}/{} H={}/{} M={}/{} L={}/{}",
        stats.triaged_critical, stats.total_critical,
        stats.triaged_high, stats.total_high,
        stats.triaged_medium, stats.total_medium,
        stats.triaged_low, stats.total_low,
    );
    Ok(stats)
}

async fn query_atlas_stats(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> anyhow::Result<(u32, u32, u32)> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;

    // Total modules
    let modules_db = &ctx.config.databases.modules;
    let total_filter = project_filter(project_id);
    let total_resp = match ctx.notion.query_database(modules_db, Some(total_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query Modules DB: {e}");
            return Ok((0, 0, 0));
        }
    };
    let total_modules = count_results(&total_resp);

    // Documented modules (Status is not empty)
    let doc_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Status", "select": { "is_not_empty": true } }
        ]
    });
    let doc_resp = match ctx.notion.query_database(modules_db, Some(doc_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query documented Modules: {e}");
            return Ok((0, total_modules, 0));
        }
    };
    let documented_modules = count_results(&doc_resp);

    // Open knowledge gaps
    let gaps_db = &ctx.config.databases.knowledge_gaps;
    let open_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Status", "select": { "does_not_equal": "Resolved" } }
        ]
    });
    let open_resp = match ctx.notion.query_database(gaps_db, Some(open_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query Knowledge Gaps: {e}");
            return Ok((documented_modules, total_modules, 0));
        }
    };
    let open_gaps = count_results(&open_resp);

    info!("[HealthAgent] Atlas: {documented_modules}/{total_modules} documented, {open_gaps} open gaps");
    Ok((documented_modules, total_modules, open_gaps))
}

async fn query_vault_stats(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> anyhow::Result<(u32, u32, u32)> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let db_id = &ctx.config.databases.env_config;

    // Total secrets
    let total_filter = project_filter(project_id);
    let total_resp = match ctx.notion.query_database(db_id, Some(total_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query Env Config DB: {e}");
            return Ok((0, 0, 0));
        }
    };
    let total_secrets = count_results(&total_resp);

    // Valid (Active) secrets
    let active_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Status", "select": { "equals": "Active" } }
        ]
    });
    let active_resp = match ctx.notion.query_database(db_id, Some(active_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query active secrets: {e}");
            return Ok((0, total_secrets, 0));
        }
    };
    let valid_secrets = count_results(&active_resp);

    // Missing in prod
    let missing_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Status", "select": { "equals": "Missing In Prod" } }
        ]
    });
    let missing_resp = match ctx.notion.query_database(db_id, Some(missing_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query missing-in-prod secrets: {e}");
            return Ok((valid_secrets, total_secrets, 0));
        }
    };
    let missing_in_prod = count_results(&missing_resp);

    info!("[HealthAgent] Vault: {valid_secrets}/{total_secrets} active, {missing_in_prod} missing in prod");
    Ok((valid_secrets, total_secrets, missing_in_prod))
}

async fn query_review_stats(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> anyhow::Result<(u32, u32, u32)> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;

    let pr_db = &ctx.config.databases.pr_reviews;

    // Merged PRs
    let merged_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Status", "select": { "equals": "Merged" } }
        ]
    });
    let merged_resp = match ctx.notion.query_database(pr_db, Some(merged_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query merged PRs: {e}");
            return Ok((0, 0, 0));
        }
    };
    let merged_prs = count_results(&merged_resp);

    // Reviewed PRs (Status == "Merged" or "Closed" — anything non-Open)
    let reviewed_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "or": [
                { "property": "Status", "select": { "equals": "Merged" } },
                { "property": "Status", "select": { "equals": "Closed" } }
            ]}
        ]
    });
    let reviewed_resp = match ctx.notion.query_database(pr_db, Some(reviewed_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query reviewed PRs: {e}");
            return Ok((0, merged_prs, 0));
        }
    };
    let reviewed_prs = count_results(&reviewed_resp);

    // Critical review patterns
    let patterns_db = &ctx.config.databases.review_patterns;
    let critical_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } },
            { "property": "Category", "select": { "equals": "Security" } }
        ]
    });
    let patterns_resp = match ctx.notion.query_database(patterns_db, Some(critical_filter), None, None).await {
        Ok(resp) => resp,
        Err(e) => {
            error!("[HealthAgent] Failed to query Review Patterns: {e}");
            return Ok((reviewed_prs, merged_prs, 0));
        }
    };
    let critical_patterns = count_results(&patterns_resp);

    info!("[HealthAgent] Review: {reviewed_prs} reviewed, {merged_prs} merged, {critical_patterns} critical patterns");
    Ok((reviewed_prs, merged_prs, critical_patterns))
}

async fn build_digest_data(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
    health_score: f64,
) -> anyhow::Result<DigestData> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;

    // Query each layer DB for this-week counts (created in last 7 days).
    // We use the built-in "Created" (created_time) property for date filtering.
    let seven_days_ago = (chrono::Utc::now() - chrono::Duration::days(7))
        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
        .to_string();

    let week_filter = |extra_conditions: Vec<serde_json::Value>| -> serde_json::Value {
        let mut conditions = vec![
            json!({ "property": "Project ID", "rich_text": { "equals": project_id } }),
            json!({ "timestamp": "created_time", "created_time": { "on_or_after": &seven_days_ago } }),
        ];
        conditions.extend(extra_conditions);
        json!({ "and": conditions })
    };

    // New RFCs this week
    let new_rfcs = match ctx.notion.query_database(
        &ctx.config.databases.rfcs, Some(week_filter(vec![])), None, None,
    ).await {
        Ok(resp) => count_results(&resp),
        Err(e) => { warn!("[HealthAgent] digest: failed to query new RFCs: {e}"); 0 }
    };

    // RFCs approved this week
    let rfcs_approved = match ctx.notion.query_database(
        &ctx.config.databases.rfcs,
        Some(week_filter(vec![
            json!({ "property": "Status", "select": { "equals": "Approved" } })
        ])),
        None, None,
    ).await {
        Ok(resp) => count_results(&resp),
        Err(e) => { warn!("[HealthAgent] digest: failed to query approved RFCs: {e}"); 0 }
    };

    // Regressions found this week
    let regressions_found = match ctx.notion.query_database(
        &ctx.config.databases.regressions, Some(week_filter(vec![])), None, None,
    ).await {
        Ok(resp) => count_results(&resp),
        Err(e) => { warn!("[HealthAgent] digest: failed to query regressions: {e}"); 0 }
    };

    // Regressions resolved this week
    let regressions_resolved = match ctx.notion.query_database(
        &ctx.config.databases.regressions,
        Some(week_filter(vec![
            json!({ "property": "Status", "select": { "equals": "Resolved" } })
        ])),
        None, None,
    ).await {
        Ok(resp) => count_results(&resp),
        Err(e) => { warn!("[HealthAgent] digest: failed to query resolved regressions: {e}"); 0 }
    };

    // New vulnerabilities this week
    let new_vulnerabilities = match ctx.notion.query_database(
        &ctx.config.databases.dependencies, Some(week_filter(vec![])), None, None,
    ).await {
        Ok(resp) => count_results(&resp),
        Err(e) => { warn!("[HealthAgent] digest: failed to query new vulns: {e}"); 0 }
    };

    // Vulnerabilities triaged this week
    let triaged_statuses: Vec<serde_json::Value> = ["Accepted Risk", "Triaged", "Resolved"]
        .iter()
        .map(|s| json!({ "property": "Triage Status", "select": { "equals": s } }))
        .collect();
    let vulnerabilities_triaged = match ctx.notion.query_database(
        &ctx.config.databases.dependencies,
        Some(week_filter(vec![json!({ "or": triaged_statuses })])),
        None, None,
    ).await {
        Ok(resp) => count_results(&resp),
        Err(e) => { warn!("[HealthAgent] digest: failed to query triaged vulns: {e}"); 0 }
    };

    // PRs reviewed this week
    let prs_reviewed = match ctx.notion.query_database(
        &ctx.config.databases.pr_reviews,
        Some(week_filter(vec![json!({
            "or": [
                { "property": "Status", "select": { "equals": "Merged" } },
                { "property": "Status", "select": { "equals": "Closed" } }
            ]
        })])),
        None, None,
    ).await {
        Ok(resp) => count_results(&resp),
        Err(e) => { warn!("[HealthAgent] digest: failed to query reviewed PRs: {e}"); 0 }
    };

    // New patterns this week
    let new_patterns = match ctx.notion.query_database(
        &ctx.config.databases.review_patterns, Some(week_filter(vec![])), None, None,
    ).await {
        Ok(resp) => count_results(&resp),
        Err(e) => { warn!("[HealthAgent] digest: failed to query new patterns: {e}"); 0 }
    };

    // Fetch last week's health report for delta computation
    let last_report_filter = json!({
        "and": [
            { "property": "Project ID", "rich_text": { "equals": project_id } }
        ]
    });
    let last_report_sorts = json!([
        { "timestamp": "created_time", "direction": "descending" }
    ]);
    let health_delta = match ctx.notion.query_database(
        &ctx.config.databases.health_reports,
        Some(last_report_filter),
        Some(last_report_sorts),
        Some(1),
    ).await {
        Ok(resp) => {
            resp["results"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|page| page["properties"]["Overall Score"]["number"].as_f64())
                .map(|prev| health_score - prev)
                .unwrap_or(0.0)
        }
        Err(e) => {
            warn!("[HealthAgent] digest: failed to fetch last health report for delta: {e}");
            0.0
        }
    };

    info!("[HealthAgent] Digest data built: delta={health_delta:.2}");

    Ok(DigestData {
        new_rfcs,
        rfcs_approved,
        regressions_found,
        regressions_resolved,
        new_vulnerabilities,
        vulnerabilities_triaged,
        prs_reviewed,
        new_patterns,
        health_score,
        health_delta,
        notable_events: Vec::new(),
        action_items: Vec::new(),
    })
}

async fn call_claude_for_narrative(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    prompt: &str,
) -> anyhow::Result<HealthNarrative> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;

    let response = ctx.claude.complete(prompts::HEALTH_NARRATIVE_SYSTEM, prompt).await?;

    // Parse the JSON response from Claude
    let narrative: HealthNarrative = serde_json::from_str(response.trim())
        .or_else(|_| {
            // Try extracting JSON from markdown code blocks
            let trimmed = response.trim();
            let json_str = if let Some(start) = trimmed.find("```json") {
                let after = &trimmed[start + 7..];
                after.find("```").map(|end| after[..end].trim()).unwrap_or(trimmed)
            } else if let Some(start) = trimmed.find('{') {
                trimmed.rfind('}').map(|end| &trimmed[start..=end]).unwrap_or(trimmed)
            } else {
                trimmed
            };
            serde_json::from_str(json_str)
        })
        .map_err(|e| {
            error!("[HealthAgent] Failed to parse Claude narrative JSON: {e}");
            anyhow::anyhow!("Failed to parse Claude narrative: {e}")
        })?;

    Ok(narrative)
}

async fn write_health_report(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
    period: &str,
    decisions_health: f64,
    pulse_health: f64,
    shield_health: f64,
    atlas_health: f64,
    vault_health: f64,
    review_health: f64,
    overall_score: f64,
    delta_from_last: f64,
    narrative: &HealthNarrative,
) -> anyhow::Result<()> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let db_id = &ctx.config.databases.health_reports;

    let report_id = format!("HR-{project_id}-{period}");
    let generated_at = chrono::Utc::now().to_rfc3339();
    let key_risks_text = narrative.key_risks.join("\n• ");
    let key_wins_text = narrative.key_wins.join("\n• ");

    let properties = json!({
        hr_schema::REPORT_ID:       prop::title(&report_id),
        hr_schema::PROJECT:         prop::rich_text(project_id),
        hr_schema::PERIOD:          prop::rich_text(period),
        hr_schema::DECISIONS_HEALTH: prop::number(decisions_health),
        hr_schema::PULSE_HEALTH:    prop::number(pulse_health),
        hr_schema::SHIELD_HEALTH:   prop::number(shield_health),
        hr_schema::ATLAS_HEALTH:    prop::number(atlas_health),
        hr_schema::VAULT_HEALTH:    prop::number(vault_health),
        hr_schema::REVIEW_HEALTH:   prop::number(review_health),
        hr_schema::OVERALL_SCORE:   prop::number(overall_score),
        hr_schema::DELTA_FROM_LAST: prop::number(delta_from_last),
        hr_schema::AI_NARRATIVE:    prop::rich_text(&narrative.narrative),
        hr_schema::KEY_RISKS:       prop::rich_text(&key_risks_text),
        hr_schema::KEY_WINS:        prop::rich_text(&key_wins_text),
        hr_schema::GENERATED_AT:    prop::date(&generated_at),
    });

    match ctx.notion.create_page(db_id, properties).await {
        Ok(resp) => {
            let page_id = resp["id"].as_str().unwrap_or("unknown");
            info!("[HealthAgent] Health Report created: {page_id}");
        }
        Err(e) => {
            error!("[HealthAgent] Failed to create Health Report: {e}");
            return Err(e);
        }
    }

    Ok(())
}

async fn write_engineering_digest(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
    digest: &DigestData,
    narrative: &str,
) -> anyhow::Result<()> {
    let ctx = get_ctx(state).ok_or_else(|| anyhow::anyhow!("Failed to downcast AgentContext"))?;
    let db_id = &ctx.config.databases.engineering_digest;

    let period = chrono::Utc::now().format("%Y-W%V").to_string();
    let digest_id = format!("ED-{project_id}-{period}");
    let generated_at = chrono::Utc::now().to_rfc3339();
    let notable_events_text = digest.notable_events.join("\n• ");
    let action_items_text = digest.action_items.join("\n• ");

    let properties = json!({
        ed_schema::DIGEST_ID:               prop::title(&digest_id),
        ed_schema::PROJECT:                 prop::rich_text(project_id),
        ed_schema::NEW_RFCS:                prop::number(digest.new_rfcs as f64),
        ed_schema::RFCS_APPROVED:           prop::number(digest.rfcs_approved as f64),
        ed_schema::REGRESSIONS_FOUND:       prop::number(digest.regressions_found as f64),
        ed_schema::REGRESSIONS_RESOLVED:    prop::number(digest.regressions_resolved as f64),
        ed_schema::NEW_VULNERABILITIES:     prop::number(digest.new_vulnerabilities as f64),
        ed_schema::VULNERABILITIES_TRIAGED: prop::number(digest.vulnerabilities_triaged as f64),
        ed_schema::PRS_REVIEWED:            prop::number(digest.prs_reviewed as f64),
        ed_schema::NEW_PATTERNS:            prop::number(digest.new_patterns as f64),
        ed_schema::HEALTH_SCORE:            prop::number(digest.health_score),
        ed_schema::HEALTH_DELTA:            prop::number(digest.health_delta),
        ed_schema::NARRATIVE:               prop::rich_text(narrative),
        ed_schema::NOTABLE_EVENTS:          prop::rich_text(&notable_events_text),
        ed_schema::ACTION_ITEMS:            prop::rich_text(&action_items_text),
        ed_schema::GENERATED_AT:            prop::date(&generated_at),
        ed_schema::SENT_TO_SLACK:           prop::checkbox(false),
    });

    match ctx.notion.create_page(db_id, properties).await {
        Ok(resp) => {
            let page_id = resp["id"].as_str().unwrap_or("unknown");
            info!("[HealthAgent] Engineering Digest created: {page_id}");
        }
        Err(e) => {
            error!("[HealthAgent] Failed to create Engineering Digest: {e}");
            return Err(e);
        }
    }

    Ok(())
}
