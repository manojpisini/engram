//! Main event loop for the engram-review agent.
//!
//! Listens for `PrOpened`, `PrMerged`, and `ReviewPatternCreated` events
//! on the broadcast channel and orchestrates the review pipeline:
//!
//! 1. Read playbook rules from Notion
//! 2. Analyze PR diff via Claude
//! 3. Extract patterns and update frequency counts
//! 4. Promote recurring patterns to Tech Debt
//! 5. Write PR Review record to Notion

use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, error, warn};

use chrono::Utc;
use serde_json::json;
use engram_types::clients::{AgentContext, properties as prop};
use engram_types::events::EngramEvent;
use engram_types::notion_schema::{
    events as events_schema, pr_reviews, review_patterns, review_playbook, tech_debt,
};

use crate::debt_tracker::{self, DEFAULT_PROMOTION_THRESHOLD};
use crate::pattern_extractor;
use crate::pr_analyzer::{self, PrContext};

// ── Helper: downcast shared state to AgentContext ──

fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

/// Run the Review-agent main loop.
pub async fn run(
    state: Arc<dyn std::any::Any + Send + Sync>,
    mut rx: broadcast::Receiver<EngramEvent>,
) {
    info!("[ReviewAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[ReviewAgent] Error handling event: {e}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[ReviewAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[ReviewAgent] Channel closed, shutting down");
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
        EngramEvent::PrOpened {
            repo,
            pr_number,
            diff,
            title,
            description,
            author,
            branch,
            target_branch,
        } => {
            handle_pr_opened(
                state,
                repo,
                *pr_number,
                diff,
                title,
                description,
                author,
                branch,
                target_branch,
            )
            .await?;
        }

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
            .await?;
        }

        EngramEvent::ReviewPatternCreated {
            pattern_notion_page_id,
            pattern_name,
            frequency,
            project_id,
        } => {
            handle_review_pattern_created(
                state,
                pattern_notion_page_id,
                pattern_name,
                *frequency,
                project_id,
            )
            .await?;
        }

        EngramEvent::SetupComplete { project_id } => {
            info!("[ReviewAgent] SetupComplete — waiting for PR data for project {project_id}");
        }

        _ => {} // Ignore events not relevant to the Review agent
    }
    Ok(())
}

// ─── PrOpened ───────────────────────────────────────────────────────

async fn handle_pr_opened(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    repo: &str,
    pr_number: u64,
    diff: &str,
    title: &str,
    description: &str,
    author: &str,
    branch: &str,
    target_branch: &str,
) -> anyhow::Result<()> {
    info!(
        "[ReviewAgent] Handling PrOpened: #{pr_number} '{title}' by {author} \
         ({branch} -> {target_branch}) in {repo}"
    );

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[ReviewAgent] No AgentContext available, skipping API calls");
            // Fall through with placeholder data so analysis still runs
            return handle_pr_opened_fallback(
                repo, pr_number, diff, title, description, author, branch, target_branch,
            )
            .await;
        }
    };

    let db_pr_reviews = &ctx.config.databases.pr_reviews;
    let db_review_playbook = &ctx.config.databases.review_playbook;
    let db_events = &ctx.config.databases.events;

    // Step 1: Read active Review Playbook rules from Notion
    let filter = json!({
        "property": review_playbook::ACTIVE,
        "checkbox": { "equals": true }
    });
    let playbook_rules_json = match ctx
        .notion
        .query_database(db_review_playbook, Some(filter), None, Some(100))
        .await
    {
        Ok(result) => {
            if let Some(pages) = result["results"].as_array() {
                let rules: Vec<serde_json::Value> = pages
                    .iter()
                    .filter_map(|p| {
                        let props = &p["properties"];
                        let rule_id = props[review_playbook::RULE_ID]["rich_text"][0]["text"]
                            ["content"]
                            .as_str()?;
                        let title = props[review_playbook::TITLE]["title"][0]["text"]["content"]
                            .as_str()?;
                        let category = props[review_playbook::CATEGORY]["select"]["name"]
                            .as_str()
                            .unwrap_or("general");
                        let description = props[review_playbook::DESCRIPTION]["rich_text"][0]
                            ["text"]["content"]
                            .as_str()
                            .unwrap_or("");
                        let severity = props[review_playbook::SEVERITY]["select"]["name"]
                            .as_str()
                            .unwrap_or("NIT");
                        Some(json!({
                            "rule_id": rule_id,
                            "title": title,
                            "category": category,
                            "description": description,
                            "severity": severity,
                            "active": true,
                        }))
                    })
                    .collect();
                serde_json::to_string(&rules).unwrap_or_else(|_| "[]".to_string())
            } else {
                "[]".to_string()
            }
        }
        Err(e) => {
            warn!("[ReviewAgent] Failed to query playbook rules: {e}");
            "[]".to_string()
        }
    };

    // Step 2: Analyze PR diff against playbook rules via Claude
    let pr_ctx = PrContext {
        repo: repo.to_string(),
        pr_number,
        title: title.to_string(),
        description: description.to_string(),
        author: author.to_string(),
        branch: branch.to_string(),
        target_branch: target_branch.to_string(),
    };

    let review_result = pr_analyzer::analyze_pr(diff, &playbook_rules_json, &pr_ctx).await?;

    // Step 3: Format review draft
    let review_draft = pr_analyzer::format_review_draft(&review_result);
    info!(
        "[ReviewAgent] Generated review draft ({} chars) for PR #{}",
        review_draft.len(),
        pr_number
    );

    // Step 4: Extract patterns and update frequency counts
    let patterns = pattern_extractor::extract_patterns(&review_result);
    info!(
        "[ReviewAgent] Extracted {} patterns from PR #{}",
        patterns.len(),
        pr_number
    );

    for pattern in &patterns {
        let freq = pattern_extractor::update_pattern_frequency(pattern, pr_number).await?;
        info!(
            "[ReviewAgent] Pattern '{}' total frequency: {}, trend: {}",
            freq.pattern_name, freq.total_frequency, freq.trend
        );

        // Step 5: Check if any pattern should be promoted to Tech Debt
        if let Some(debt) = debt_tracker::check_debt_promotion(
            &freq.pattern_name,
            freq.total_frequency,
            DEFAULT_PROMOTION_THRESHOLD,
            &freq.category,
        )
        .await?
        {
            info!(
                "[ReviewAgent] Promoted pattern '{}' to Tech Debt: '{}'",
                freq.pattern_name, debt.title
            );

            // Create tech debt record in Notion
            let db_tech_debt = &ctx.config.databases.tech_debt;
            let now = Utc::now().to_rfc3339();
            let debt_props = json!({
                tech_debt::DEBT_ITEM: prop::title(&debt.title),
                tech_debt::SOURCE: prop::select("Review Pattern"),
                tech_debt::SEVERITY: prop::select(&debt.severity),
                tech_debt::STATUS: prop::select("Open"),
                tech_debt::IDENTIFIED_AT: prop::date(&now),
            });
            match ctx.notion.create_page(db_tech_debt, debt_props).await {
                Ok(_) => info!("[ReviewAgent] Created tech debt item: '{}'", debt.title),
                Err(e) => error!("[ReviewAgent] Failed to create tech debt item: {e}"),
            }
        }
    }

    // Step 6: Write PR Review record to Notion with status "Draft"
    let now = Utc::now().to_rfc3339();
    let review_props = json!({
        pr_reviews::PR_ID: prop::title(&format!("#{pr_number}")),
        pr_reviews::TITLE: prop::rich_text(title),
        pr_reviews::AUTHOR: prop::rich_text(author),
        pr_reviews::BRANCH: prop::rich_text(branch),
        pr_reviews::TARGET_BRANCH: prop::rich_text(target_branch),
        pr_reviews::STATUS: prop::select("Open"),
        pr_reviews::BLOCKER_COUNT: prop::number(review_result.blockers.len() as f64),
        pr_reviews::SUGGESTION_COUNT: prop::number(review_result.suggestions.len() as f64),
        pr_reviews::NIT_COUNT: prop::number(review_result.nits.len() as f64),
        pr_reviews::CLAUDE_REVIEW_DRAFT: prop::rich_text(&review_draft),
        pr_reviews::REVIEW_DRAFT_STATUS: prop::select("Draft"),
        pr_reviews::REVIEW_QUALITY_SCORE: prop::number(review_result.quality_score.into()),
        pr_reviews::OPENED_AT: prop::date(&now),
    });
    match ctx.notion.create_page(db_pr_reviews, review_props).await {
        Ok(_) => info!("[ReviewAgent] Created PR Review record for PR #{pr_number}"),
        Err(e) => error!("[ReviewAgent] Failed to create PR Review record: {e}"),
    }

    // Create timeline event
    let event_props = json!({
        events_schema::TITLE: prop::title(&format!("PR #{pr_number} review generated")),
        events_schema::TYPE: prop::rich_text("PrReviewed"),
        events_schema::SOURCE_LAYER: prop::select("Review"),
        events_schema::DETAILS: prop::rich_text(&format!("pr={repo}#{pr_number}")),
        events_schema::TIMESTAMP: prop::date(&now),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[ReviewAgent] Created timeline event for PR #{pr_number}"),
        Err(e) => error!("[ReviewAgent] Failed to create timeline event: {e}"),
    }

    info!("[ReviewAgent] PrOpened handling complete for PR #{pr_number}");
    Ok(())
}

/// Fallback when no AgentContext is available (e.g. in tests).
async fn handle_pr_opened_fallback(
    repo: &str,
    pr_number: u64,
    diff: &str,
    title: &str,
    description: &str,
    author: &str,
    branch: &str,
    target_branch: &str,
) -> anyhow::Result<()> {
    let playbook_rules_json = "[]";
    let pr_ctx = PrContext {
        repo: repo.to_string(),
        pr_number,
        title: title.to_string(),
        description: description.to_string(),
        author: author.to_string(),
        branch: branch.to_string(),
        target_branch: target_branch.to_string(),
    };

    let review_result = pr_analyzer::analyze_pr(diff, playbook_rules_json, &pr_ctx).await?;
    let review_draft = pr_analyzer::format_review_draft(&review_result);
    info!(
        "[ReviewAgent] Generated review draft ({} chars) for PR #{} (no context)",
        review_draft.len(),
        pr_number
    );

    let patterns = pattern_extractor::extract_patterns(&review_result);
    for pattern in &patterns {
        let _freq = pattern_extractor::update_pattern_frequency(pattern, pr_number).await?;
    }

    Ok(())
}

// ─── PrMerged ───────────────────────────────────────────────────────

async fn handle_pr_merged(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    repo: &str,
    pr_number: u64,
    diff: &str,
    branch: &str,
    commit_sha: &str,
    title: &str,
    author: &str,
    rfc_references: &[String],
) -> anyhow::Result<()> {
    info!(
        "[ReviewAgent] Handling PrMerged: #{pr_number} '{title}' by {author} \
         (commit {commit_sha}) in {repo}"
    );

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[ReviewAgent] No AgentContext available, skipping API calls for PrMerged");
            return Ok(());
        }
    };

    let db_pr_reviews = &ctx.config.databases.pr_reviews;
    let db_events = &ctx.config.databases.events;

    // Step 1: Create / update PR Review record with status "Merged"
    let now = Utc::now().to_rfc3339();
    let review_props = json!({
        pr_reviews::PR_ID: prop::title(&format!("#{pr_number}")),
        pr_reviews::TITLE: prop::rich_text(title),
        pr_reviews::AUTHOR: prop::rich_text(author),
        pr_reviews::BRANCH: prop::rich_text(branch),
        pr_reviews::STATUS: prop::select("Merged"),
        pr_reviews::MERGED_AT: prop::date(&now),
    });
    match ctx.notion.create_page(db_pr_reviews, review_props).await {
        Ok(_) => info!("[ReviewAgent] Created/updated PR Review record for merged PR #{pr_number}"),
        Err(e) => error!("[ReviewAgent] Failed to create PR Review record: {e}"),
    }

    // Step 2: Extract patterns from the merged diff
    let pr_ctx = PrContext {
        repo: repo.to_string(),
        pr_number,
        title: title.to_string(),
        description: String::new(),
        author: author.to_string(),
        branch: branch.to_string(),
        target_branch: String::new(),
    };

    let playbook_rules_json = "[]"; // Merged analysis uses lighter rules
    let review_result = pr_analyzer::analyze_pr(diff, playbook_rules_json, &pr_ctx).await?;
    let patterns = pattern_extractor::extract_patterns(&review_result);

    info!(
        "[ReviewAgent] Extracted {} patterns from merged PR #{}",
        patterns.len(),
        pr_number
    );

    for pattern in &patterns {
        let _freq = pattern_extractor::update_pattern_frequency(pattern, pr_number).await?;
    }

    // Step 3: Link to RFC if referenced
    if !rfc_references.is_empty() {
        info!(
            "[ReviewAgent] PR #{pr_number} references RFCs: {:?}",
            rfc_references
        );

        // Query for the existing PR review record
        let filter = json!({
            "property": pr_reviews::PR_ID,
            "title": { "equals": format!("#{pr_number}") }
        });
        match ctx.notion.query_database(db_pr_reviews, Some(filter), None, Some(1)).await {
            Ok(result) => {
                if let Some(pages) = result["results"].as_array() {
                    for page in pages {
                        if let Some(page_id) = page["id"].as_str() {
                            for rfc_ref in rfc_references {
                                let update_props = json!({
                                    pr_reviews::IMPLEMENTS_RFC: prop::rich_text(rfc_ref),
                                });
                                match ctx.notion.update_page(page_id, update_props).await {
                                    Ok(_) => info!("[ReviewAgent] Linked PR #{pr_number} to RFC {rfc_ref}"),
                                    Err(e) => error!("[ReviewAgent] Failed to link PR to RFC {rfc_ref}: {e}"),
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => error!("[ReviewAgent] Failed to query PR Reviews for #{pr_number}: {e}"),
        }
    }

    // Create timeline event
    let event_props = json!({
        events_schema::TITLE: prop::title(&format!("PR #{pr_number} merged")),
        events_schema::TYPE: prop::rich_text("PrMerged"),
        events_schema::SOURCE_LAYER: prop::select("Review"),
        events_schema::DETAILS: prop::rich_text(&format!("pr={repo}#{pr_number}")),
        events_schema::TIMESTAMP: prop::date(&now),
    });
    match ctx.notion.create_page(db_events, event_props).await {
        Ok(_) => info!("[ReviewAgent] Created merged event for PR #{pr_number}"),
        Err(e) => error!("[ReviewAgent] Failed to create merged event: {e}"),
    }

    info!("[ReviewAgent] PrMerged handling complete for PR #{pr_number}");
    Ok(())
}

// ─── ReviewPatternCreated ───────────────────────────────────────────

async fn handle_review_pattern_created(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    pattern_notion_page_id: &str,
    pattern_name: &str,
    frequency: u32,
    project_id: &str,
) -> anyhow::Result<()> {
    info!(
        "[ReviewAgent] Handling ReviewPatternCreated: '{}' (freq={}, project={}, page={})",
        pattern_name, frequency, project_id, pattern_notion_page_id
    );

    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            warn!("[ReviewAgent] No AgentContext available, skipping API calls for ReviewPatternCreated");
            // Still check promotion with placeholder category
            let category = "unknown";
            if let Some(debt) = debt_tracker::check_debt_promotion(
                pattern_name,
                frequency,
                DEFAULT_PROMOTION_THRESHOLD,
                category,
            )
            .await?
            {
                info!(
                    "[ReviewAgent] Pattern '{}' promoted to Tech Debt: '{}' (no context)",
                    pattern_name, debt.title
                );
            }
            return Ok(());
        }
    };

    let db_review_patterns = &ctx.config.databases.review_patterns;
    let db_tech_debt = &ctx.config.databases.tech_debt;
    let db_events = &ctx.config.databases.events;

    // Read the full pattern record from Notion to get the category
    let filter = json!({
        "property": review_patterns::PATTERN_NAME,
        "title": { "equals": pattern_name }
    });
    let category = match ctx
        .notion
        .query_database(db_review_patterns, Some(filter), None, Some(1))
        .await
    {
        Ok(result) => {
            result["results"][0]["properties"][review_patterns::CATEGORY]["select"]["name"]
                .as_str()
                .unwrap_or("unknown")
                .to_string()
        }
        Err(e) => {
            warn!("[ReviewAgent] Failed to read pattern record: {e}");
            "unknown".to_string()
        }
    };

    // Check for Tech Debt promotion
    if let Some(debt) = debt_tracker::check_debt_promotion(
        pattern_name,
        frequency,
        DEFAULT_PROMOTION_THRESHOLD,
        &category,
    )
    .await?
    {
        info!(
            "[ReviewAgent] Pattern '{}' promoted to Tech Debt: '{}'",
            pattern_name, debt.title
        );

        // Create the tech debt item in Notion
        let now = Utc::now().to_rfc3339();
        let debt_props = json!({
            tech_debt::DEBT_ITEM: prop::title(&debt.title),
            tech_debt::SOURCE: prop::select("Review Pattern"),
            tech_debt::SEVERITY: prop::select(&debt.severity),
            tech_debt::PROJECT: prop::rich_text(project_id),
            tech_debt::STATUS: prop::select("Open"),
            tech_debt::IDENTIFIED_AT: prop::date(&now),
        });
        match ctx.notion.create_page(db_tech_debt, debt_props).await {
            Ok(debt_page) => {
                let debt_page_id = debt_page["id"].as_str().unwrap_or("");
                info!("[ReviewAgent] Created tech debt item: '{}' (page={})", debt.title, debt_page_id);

                // Link the Tech Debt item back to the pattern
                let link_props = json!({
                    review_patterns::TECH_DEBT_ITEM: prop::relation(&[debt_page_id]),
                    review_patterns::TREND: prop::select("Increasing"),
                });
                match ctx.notion.update_page(pattern_notion_page_id, link_props).await {
                    Ok(_) => info!("[ReviewAgent] Linked pattern to tech debt item"),
                    Err(e) => error!("[ReviewAgent] Failed to link pattern to tech debt: {e}"),
                }
            }
            Err(e) => error!("[ReviewAgent] Failed to create tech debt item: {e}"),
        }

        // Create timeline event
        let now = Utc::now().to_rfc3339();
        let event_props = json!({
            events_schema::TITLE: prop::title(&format!("Pattern '{}' promoted to tech debt", pattern_name)),
            events_schema::TYPE: prop::rich_text("TechDebtCreated"),
            events_schema::SOURCE_LAYER: prop::select("Review"),
            events_schema::PROJECT: prop::rich_text(project_id),
            events_schema::TIMESTAMP: prop::date(&now),
        });
        match ctx.notion.create_page(db_events, event_props).await {
            Ok(_) => info!("[ReviewAgent] Created tech debt promotion event"),
            Err(e) => error!("[ReviewAgent] Failed to create tech debt event: {e}"),
        }
    }

    info!(
        "[ReviewAgent] ReviewPatternCreated handling complete for '{}'",
        pattern_name
    );
    Ok(())
}
