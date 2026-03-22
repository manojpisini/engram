//! Atlas agent main event loop: module documentation and onboarding.
//!
//! Handles:
//! - `PrMerged` — re-generate module summary, update key fields.
//! - `RfcApproved` — flag affected modules for doc update.
//! - `NewEngineerOnboards` — generate an onboarding track for their role.
//! - `WeeklyKnowledgeGapTrigger` — detect undocumented modules, stale docs, orphaned RFCs.

use std::sync::Arc;

use anyhow::Result;
use chrono::Utc;
use serde_json::json;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use engram_types::clients::{AgentContext, properties as prop};
use engram_types::events::EngramEvent;
use engram_types::notion_schema::{
    events as events_schema, knowledge_gaps, modules, onboarding_steps, onboarding_tracks,
};

use crate::gap_detector::{
    detect_orphaned_rfcs, detect_stale_docs, detect_undocumented_modules, GapSeverity,
    KnowledgeGap, ModuleInfo, RfcInfo,
};
use crate::module_summarizer;
use crate::onboarding_generator;
use crate::prompts::ModuleSummaryContext;

// ── Helper: downcast shared state to AgentContext ──

fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

// ── Public entry point ──

/// Run the Atlas agent main event loop.
///
/// `state` is an opaque shared-state handle (`Arc<dyn Any + Send + Sync>`) that
/// downstream callers can downcast to their concrete application state containing
/// Notion MCP clients, Claude API clients, and configuration.
pub async fn run(
    state: Arc<dyn std::any::Any + Send + Sync>,
    mut rx: broadcast::Receiver<EngramEvent>,
) {
    info!("[AtlasAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[AtlasAgent] Error handling event: {e:#}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[AtlasAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[AtlasAgent] Channel closed, shutting down");
                break;
            }
        }
    }
}

// ── Event dispatch ──

async fn handle_event(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    event: &EngramEvent,
) -> Result<()> {
    match event {
        EngramEvent::PrMerged {
            repo,
            pr_number,
            diff,
            branch,
            commit_sha,
            title,
            author,
            rfc_references,
        } => {
            handle_pr_merged(
                state,
                repo,
                *pr_number,
                diff,
                branch,
                commit_sha,
                title,
                author,
                rfc_references,
            )
            .await
        }

        EngramEvent::RfcApproved {
            rfc_notion_page_id,
            rfc_id,
            project_id,
            affected_modules,
            required_env_vars,
            banned_patterns: _,
        } => {
            handle_rfc_approved(
                state,
                rfc_notion_page_id,
                rfc_id,
                project_id,
                affected_modules,
                required_env_vars,
            )
            .await
        }

        EngramEvent::NewEngineerOnboards {
            engineer_name,
            role,
            project_id,
            repo,
        } => handle_new_engineer(state, engineer_name, role, project_id, repo).await,

        EngramEvent::WeeklyKnowledgeGapTrigger { project_id } => {
            handle_knowledge_gap_scan(state, project_id).await
        }

        EngramEvent::SetupComplete { project_id } => {
            info!("[AtlasAgent] SetupComplete — generating knowledge gap scan");
            handle_knowledge_gap_scan(state, project_id).await
            // Onboarding docs are generated per-repo via NewEngineerOnboards events
        }

        _ => Ok(()), // Ignore events not relevant to Atlas
    }
}

// ── PrMerged: re-generate module summary ──

#[allow(clippy::too_many_arguments)]
async fn handle_pr_merged(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    repo: &str,
    pr_number: u64,
    diff: &str,
    branch: &str,
    commit_sha: &str,
    title: &str,
    _author: &str,
    rfc_references: &[String],
) -> Result<()> {
    info!(
        "[AtlasAgent] PrMerged: repo={repo} pr=#{pr_number} branch={branch} sha={commit_sha} title={title:?}"
    );

    // Step 1: Determine which modules are affected by this PR.
    let affected_modules = extract_affected_modules_from_diff(diff);

    if affected_modules.is_empty() {
        info!("[AtlasAgent] No modules affected by PR #{pr_number}, skipping summarization");
        return Ok(());
    }

    info!(
        "[AtlasAgent] PR #{pr_number} affects {} module(s): {:?}",
        affected_modules.len(),
        affected_modules
    );

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[AtlasAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_modules = &ctx.config.databases.modules;
    let db_events = &ctx.config.databases.events;

    // Step 2: For each affected module, build a summarization prompt and
    // send it to Claude, then update Notion.
    for module_name in &affected_modules {
        info!(
            "[AtlasAgent] Generating summary for module={module_name} triggered by PR #{pr_number}"
        );

        let prompt = module_summarizer::build_summarization_prompt(
            module_name,
            module_name, // path — in production, fetched from Notion
            &[],         // key_files — in production, fetched from Notion
            Some(diff),
        );

        let system = "You are a code documentation expert. Respond with valid JSON only.";
        match ctx.claude.complete(system, &prompt).await {
            Ok(response) => {
                match module_summarizer::parse_module_summary(&response) {
                    Ok(summary) => {
                        info!(
                            "[AtlasAgent] Parsed summary for module={module_name}: complexity={}",
                            summary.complexity_score
                        );
                        let now = Utc::now().to_rfc3339();
                        let props = json!({
                            modules::MODULE_NAME: prop::title(module_name),
                            modules::SUMMARY: prop::rich_text(&summary.what_it_does),
                            modules::COMPLEXITY_SCORE: prop::number(summary.complexity_score as f64),
                            modules::LAST_UPDATED: prop::date(&now),
                        });
                        match ctx.notion.create_page(db_modules, props).await {
                            Ok(_) => info!("[AtlasAgent] Updated module={module_name} in Notion"),
                            Err(e) => error!("[AtlasAgent] Failed to update module={module_name}: {e}"),
                        }
                    }
                    Err(e) => error!("[AtlasAgent] Failed to parse summary for module={module_name}: {e}"),
                }
            }
            Err(e) => error!("[AtlasAgent] Claude call failed for module={module_name}: {e}"),
        }

        // Step 3: Log RFC cross-references by creating timeline events.
        if !rfc_references.is_empty() {
            for rfc_ref in rfc_references {
                let now = Utc::now().to_rfc3339();
                let event_props = json!({
                    events_schema::TITLE: prop::title(&format!("Module {module_name} updated via PR #{pr_number}, linked to {rfc_ref}")),
                    events_schema::TYPE: prop::rich_text("ModuleUpdated"),
                    events_schema::SOURCE_LAYER: prop::select("Atlas"),
                    events_schema::DETAILS: prop::rich_text(&format!("rfc={rfc_ref}")),
                    events_schema::TIMESTAMP: prop::date(&now),
                });
                match ctx.notion.create_page(db_events, event_props).await {
                    Ok(_) => info!("[AtlasAgent] Created event linking module={module_name} to RFC {rfc_ref}"),
                    Err(e) => error!("[AtlasAgent] Failed to create RFC link event: {e}"),
                }
            }
        }
    }

    info!("[AtlasAgent] PrMerged handling complete for PR #{pr_number}");
    Ok(())
}

/// Extract module names from a diff by looking at path prefixes.
///
/// This is a heuristic: it looks for paths like `crates/<name>/` or `src/<name>/`.
/// In production, this would be replaced by querying the Modules database.
fn extract_affected_modules_from_diff(diff: &str) -> Vec<String> {
    let mut modules = std::collections::HashSet::new();
    let re = regex::Regex::new(r"(?:^|\n)(?:diff --git a/|[+-]{3} [ab]/)(?:crates/|src/)([^/]+)/")
        .expect("valid regex");

    for cap in re.captures_iter(diff) {
        if let Some(m) = cap.get(1) {
            modules.insert(m.as_str().to_string());
        }
    }
    modules.into_iter().collect()
}

// ── RfcApproved: flag affected modules for doc update ──

async fn handle_rfc_approved(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    _rfc_notion_page_id: &str,
    rfc_id: &str,
    project_id: &str,
    affected_modules: &[String],
    required_env_vars: &[String],
) -> Result<()> {
    info!(
        "[AtlasAgent] RfcApproved: rfc={rfc_id} project={project_id} affected_modules={affected_modules:?}"
    );

    if affected_modules.is_empty() {
        info!("[AtlasAgent] RFC {rfc_id} has no affected modules, nothing to flag");
        return Ok(());
    }

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[AtlasAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_modules = &ctx.config.databases.modules;
    let db_events = &ctx.config.databases.events;

    // Flag each affected module for documentation update in Notion.
    for module_name in affected_modules {
        info!(
            "[AtlasAgent] Flagging module={module_name} for doc update due to RFC {rfc_id}"
        );

        // Query modules DB to find the module page, then update its status
        let filter = json!({
            "property": modules::MODULE_NAME,
            "title": { "equals": module_name }
        });
        match ctx.notion.query_database(db_modules, Some(filter), None, Some(1)).await {
            Ok(result) => {
                if let Some(pages) = result["results"].as_array() {
                    for page in pages {
                        if let Some(page_id) = page["id"].as_str() {
                            let update_props = json!({
                                modules::STATUS: prop::select("Needs Doc Update"),
                            });
                            match ctx.notion.update_page(page_id, update_props).await {
                                Ok(_) => info!("[AtlasAgent] Flagged module={module_name} for doc update"),
                                Err(e) => error!("[AtlasAgent] Failed to flag module={module_name}: {e}"),
                            }
                        }
                    }
                }
            }
            Err(e) => error!("[AtlasAgent] Failed to query modules for {module_name}: {e}"),
        }
    }

    // If the RFC introduces new env vars, log a timeline event.
    if !required_env_vars.is_empty() {
        let now = Utc::now().to_rfc3339();
        let event_props = json!({
            events_schema::TITLE: prop::title(&format!("RFC {rfc_id} introduces env vars: {}", required_env_vars.join(", "))),
            events_schema::TYPE: prop::rich_text("RfcApproved"),
            events_schema::SOURCE_LAYER: prop::select("Atlas"),
            events_schema::PROJECT: prop::rich_text(project_id),
            events_schema::DETAILS: prop::rich_text(&format!("rfc={rfc_id}")),
            events_schema::TIMESTAMP: prop::date(&now),
        });
        match ctx.notion.create_page(db_events, event_props).await {
            Ok(_) => info!("[AtlasAgent] Created event for RFC {rfc_id} env vars"),
            Err(e) => error!("[AtlasAgent] Failed to create RFC env vars event: {e}"),
        }
    }

    info!("[AtlasAgent] RfcApproved handling complete for {rfc_id}");
    Ok(())
}

// ── NewEngineerOnboards: generate onboarding track ──

async fn handle_new_engineer(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    engineer_name: &str,
    role: &engram_types::events::Role,
    project_id: &str,
    repo: &str,
) -> Result<()> {
    let role_str = role.to_string();
    info!(
        "[AtlasAgent] NewEngineerOnboards: engineer={engineer_name} role={role_str} repo={repo}"
    );

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[AtlasAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_modules = &ctx.config.databases.modules;
    let db_onboarding_tracks = &ctx.config.databases.onboarding_tracks;
    let db_onboarding_steps = &ctx.config.databases.onboarding_steps;
    let db_events = &ctx.config.databases.events;

    // Step 1: Fetch module summaries from Notion.
    let module_summaries: Vec<ModuleSummaryContext> = match ctx
        .notion
        .query_database(db_modules, None, None, Some(100))
        .await
    {
        Ok(result) => {
            if let Some(pages) = result["results"].as_array() {
                pages
                    .iter()
                    .filter_map(|p| {
                        let props = &p["properties"];
                        let name = props[modules::MODULE_NAME]["title"][0]["text"]["content"]
                            .as_str()?
                            .to_string();
                        let what_it_does = props[modules::SUMMARY]["rich_text"][0]["text"]
                            ["content"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let complexity_score = props[modules::COMPLEXITY_SCORE]["number"]
                            .as_f64()
                            .unwrap_or(0.0) as u8;
                        Some(ModuleSummaryContext {
                            name,
                            what_it_does,
                            complexity_score,
                        })
                    })
                    .collect()
            } else {
                vec![]
            }
        }
        Err(e) => {
            warn!("[AtlasAgent] Failed to fetch modules: {e}");
            vec![]
        }
    };

    // Step 2: Placeholder env vars and RFC titles (could be fetched from other DBs).
    let env_vars: Vec<String> = vec![];
    let recent_rfc_titles: Vec<String> = vec![];

    // Step 3: Build prompt and send to Claude.
    // Use the repo name (e.g. "owner/repo") so the onboarding doc is about the
    // tracked repository, not ENGRAM itself.
    let repo_label = if repo.is_empty() { project_id } else { repo };
    let prompt = onboarding_generator::build_onboarding_prompt(
        &role_str,
        repo_label,
        &module_summaries,
        &env_vars,
        &recent_rfc_titles,
    );

    let system = "You are an engineering onboarding expert. Respond with valid JSON only.";
    match ctx.claude.complete(system, &prompt).await {
        Ok(response) => {
            match onboarding_generator::parse_onboarding_track(&response, &role_str) {
                Ok(track) => {
                    info!(
                        "[AtlasAgent] Generated onboarding track: {} ({} steps, ~{}h)",
                        track.name,
                        track.steps.len(),
                        track.estimated_hours
                    );

                    // Create the onboarding track in Notion
                    let now = Utc::now().to_rfc3339();
                    let track_props = json!({
                        onboarding_tracks::TRACK_NAME: prop::title(&track.name),
                        onboarding_tracks::PROJECT: prop::rich_text(project_id),
                        onboarding_tracks::ROLE: prop::select(&role_str),
                        onboarding_tracks::ESTIMATED_HOURS: prop::number(track.estimated_hours as f64),
                        onboarding_tracks::STEP_COUNT: prop::number(track.steps.len() as f64),
                        onboarding_tracks::LAST_UPDATED: prop::date(&now),
                        onboarding_tracks::UPDATED_BY: prop::rich_text("Atlas Agent"),
                    });
                    match ctx.notion.create_page(db_onboarding_tracks, track_props).await {
                        Ok(track_page) => {
                            let track_page_id = track_page["id"].as_str().unwrap_or("");
                            info!("[AtlasAgent] Created onboarding track page: {track_page_id}");

                            // Create each step as a child page
                            for step in &track.steps {
                                let step_props = json!({
                                    onboarding_steps::STEP_TITLE: prop::title(&step.title),
                                    onboarding_steps::TRACK: prop::relation(&[track_page_id]),
                                    onboarding_steps::WEEK_DAY: prop::rich_text(&step.week_day),
                                    onboarding_steps::TYPE: prop::select(&step.step_type.to_string()),
                                    onboarding_steps::DESCRIPTION: prop::rich_text(&step.description),
                                    onboarding_steps::ESTIMATED_TIME: prop::rich_text(&step.estimated_time),
                                    onboarding_steps::AUTO_GENERATED: prop::checkbox(true),
                                });
                                match ctx.notion.create_page(db_onboarding_steps, step_props).await {
                                    Ok(_) => info!("[AtlasAgent] Created onboarding step: {}", step.title),
                                    Err(e) => error!("[AtlasAgent] Failed to create step '{}': {e}", step.title),
                                }
                            }
                        }
                        Err(e) => error!("[AtlasAgent] Failed to create onboarding track: {e}"),
                    }

                    // Create timeline event
                    let now = Utc::now().to_rfc3339();
                    let event_props = json!({
                        events_schema::TITLE: prop::title(&format!("Onboarding track generated for {engineer_name} ({role_str})")),
                        events_schema::TYPE: prop::rich_text("OnboardingGenerated"),
                        events_schema::SOURCE_LAYER: prop::select("Atlas"),
                        events_schema::PROJECT: prop::rich_text(project_id),
                        events_schema::TIMESTAMP: prop::date(&now),
                    });
                    match ctx.notion.create_page(db_events, event_props).await {
                        Ok(_) => info!("[AtlasAgent] Created onboarding event for {engineer_name}"),
                        Err(e) => error!("[AtlasAgent] Failed to create onboarding event: {e}"),
                    }
                }
                Err(e) => error!("[AtlasAgent] Failed to parse onboarding track: {e}"),
            }
        }
        Err(e) => error!("[AtlasAgent] Claude call failed for onboarding: {e}"),
    }

    info!(
        "[AtlasAgent] NewEngineerOnboards handling complete for {engineer_name} ({role_str})"
    );
    Ok(())
}

// ── WeeklyKnowledgeGapTrigger: detect knowledge gaps ──

async fn handle_knowledge_gap_scan(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    project_id: &str,
) -> Result<()> {
    info!("[AtlasAgent] WeeklyKnowledgeGapTrigger: project={project_id}");

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[AtlasAgent] No AgentContext available, skipping API calls");
            return Ok(());
        }
    };

    let db_modules = &ctx.config.databases.modules;
    let db_knowledge_gaps = &ctx.config.databases.knowledge_gaps;
    let db_events = &ctx.config.databases.events;

    // Step 1: Fetch all modules from Notion.
    let modules_list: Vec<ModuleInfo> = match ctx
        .notion
        .query_database(db_modules, None, None, Some(100))
        .await
    {
        Ok(result) => {
            if let Some(pages) = result["results"].as_array() {
                pages
                    .iter()
                    .filter_map(|p| {
                        let props = &p["properties"];
                        let name = props[modules::MODULE_NAME]["title"][0]["text"]["content"]
                            .as_str()?
                            .to_string();
                        let what_it_does = props[modules::SUMMARY]["rich_text"][0]["text"]
                            ["content"]
                            .as_str()
                            .map(|s| s.to_string());
                        let last_updated = props[modules::LAST_UPDATED]["date"]
                            ["start"]
                            .as_str()
                            .and_then(|s| s.parse().ok());
                        Some(ModuleInfo {
                            name,
                            what_it_does,
                            last_updated,
                            related_rfcs: vec![],
                        })
                    })
                    .collect()
            } else {
                vec![]
            }
        }
        Err(e) => {
            warn!("[AtlasAgent] Failed to fetch modules: {e}");
            vec![]
        }
    };

    // Step 2: Placeholder RFCs (could be fetched from RFCs DB).
    let rfcs: Vec<RfcInfo> = vec![];

    // Step 3: Run all three gap detectors.
    let undocumented = detect_undocumented_modules(&modules_list);
    let stale = detect_stale_docs(&modules_list, 90);
    let orphaned = detect_orphaned_rfcs(&rfcs, &modules_list);

    let all_gaps: Vec<&KnowledgeGap> = undocumented
        .iter()
        .chain(stale.iter())
        .chain(orphaned.iter())
        .collect();

    info!(
        "[AtlasAgent] Knowledge gap scan found {} gap(s): undocumented={}, stale={}, orphaned={}",
        all_gaps.len(),
        undocumented.len(),
        stale.len(),
        orphaned.len()
    );

    // Step 4: Write each gap to the Knowledge Gaps database in Notion.
    for gap in &all_gaps {
        let mut details_parts: Vec<String> = vec![
            format!("type={}", gap.gap_type),
            "detected_by=Atlas Agent".to_string(),
        ];
        if let Some(ref module) = gap.related_module {
            details_parts.push(format!("module={module}"));
        }
        if let Some(ref rfc) = gap.related_rfc {
            details_parts.push(format!("rfc={rfc}"));
        }

        let gap_props = json!({
            knowledge_gaps::GAP_TITLE: prop::title(&gap.title),
            knowledge_gaps::SEVERITY: prop::select(&gap.severity.to_string()),
            knowledge_gaps::STATUS: prop::select("Open"),
            knowledge_gaps::DESCRIPTION: prop::rich_text(&details_parts.join(" | ")),
        });

        match ctx.notion.create_page(db_knowledge_gaps, gap_props).await {
            Ok(_) => info!("[AtlasAgent] Created knowledge gap: {:?}", gap.title),
            Err(e) => error!("[AtlasAgent] Failed to create knowledge gap '{}': {e}", gap.title),
        }
    }

    // Step 5: Log summary for observability.
    let high_count = all_gaps
        .iter()
        .filter(|g| g.severity == GapSeverity::High)
        .count();
    if high_count > 0 {
        warn!(
            "[AtlasAgent] {high_count} HIGH severity knowledge gap(s) detected for project {project_id}"
        );

        // Create a timeline event for high-severity gaps
        let now = Utc::now().to_rfc3339();
        let event_props = json!({
            events_schema::TITLE: prop::title(&format!("{high_count} high-severity knowledge gaps detected")),
            events_schema::TYPE: prop::rich_text("KnowledgeGapDetected"),
            events_schema::SOURCE_LAYER: prop::select("Atlas"),
            events_schema::PROJECT: prop::rich_text(project_id),
            events_schema::DETAILS: prop::rich_text("severity=High"),
            events_schema::TIMESTAMP: prop::date(&now),
        });
        match ctx.notion.create_page(db_events, event_props).await {
            Ok(_) => info!("[AtlasAgent] Created high-severity gap event"),
            Err(e) => error!("[AtlasAgent] Failed to create gap event: {e}"),
        }
    }

    info!("[AtlasAgent] WeeklyKnowledgeGapTrigger handling complete for {project_id}");
    Ok(())
}
