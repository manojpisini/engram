//! Setup logic for ENGRAM — creates all Notion databases, relations,
//! playbook rules, and sample data. Called by the dashboard setup wizard
//! via `POST /api/setup/notion`.

use anyhow::{Context, Result};
use serde_json::json;
use tracing::info;

use crate::notion_client::NotionMcpClient;
use engram_types::config::DatabaseIds;

/// Progress callback type — receives (step_number, total_steps, message)
pub type ProgressFn = Box<dyn Fn(usize, usize, &str) + Send + Sync>;

/// Create all 23 ENGRAM databases in the Notion workspace.
/// Automatically creates (or reuses) a styled ENGRAM parent page at the workspace root,
/// then creates all databases as children. No manual page ID required.
/// Returns the populated DatabaseIds and the parent page ID.
pub async fn create_all_databases(
    notion: &NotionMcpClient,
    _parent_id: &str,
) -> Result<DatabaseIds> {
    let mut db = DatabaseIds::default();
    let total = 23;
    let mut step = 0;

    // Auto-create or find the ENGRAM parent page at workspace root
    let engram_page_id = find_or_create_engram_page(notion).await
        .context("Failed to create ENGRAM parent page. Make sure your Notion integration token is valid and has full access to the workspace.")?;
    info!("[Setup] Using ENGRAM parent page: {}", engram_page_id);

    macro_rules! create_db {
        ($name:expr, $field:ident, $props:expr) => {{
            step += 1;
            info!("[Setup] ({}/{}) Creating {}...", step, total, $name);
            let result = notion.create_database($name, &engram_page_id, $props).await
                .with_context(|| format!("Failed to create database: {}", $name))?;
            db.$field = extract_id(&result);
            info!("[Setup] ({}/{}) {} → {}", step, total, $name, &db.$field);
        }};
    }

    // 1. Projects (Anchor)
    create_db!("ENGRAM/Projects", projects, json!({
        "Name": { "title": {} },
        "Description": { "rich_text": {} },
        "Repo URL": { "url": {} },
        "Status": { "select": { "options": [
            { "name": "Active", "color": "green" },
            { "name": "Archived", "color": "gray" },
            { "name": "Planning", "color": "yellow" }
        ]}},
        "Created At": { "date": {} }
    }));

    // 2. RFCs
    create_db!("ENGRAM/RFCs", rfcs, json!({
        "RFC ID": { "title": {} }, "Title": { "rich_text": {} },
        "Status": { "select": { "options": [
            { "name": "Draft", "color": "gray" }, { "name": "Under Review", "color": "yellow" },
            { "name": "Approved", "color": "green" }, { "name": "Implementing", "color": "blue" },
            { "name": "Implemented", "color": "purple" }, { "name": "Deprecated", "color": "red" }
        ]}},
        "Problem Statement": { "rich_text": {} }, "Proposed Solution": { "rich_text": {} },
        "Alternatives Considered": { "rich_text": {} }, "Trade-offs": { "rich_text": {} },
        "Decision Rationale": { "rich_text": {} }, "Author": { "rich_text": {} },
        "Reviewers": { "rich_text": {} }, "Opens": { "date": {} }, "Resolves": { "date": {} },
        "Drift Notes": { "rich_text": {} }, "RFC Drift Score": { "number": { "format": "number" } }
    }));

    // 3. RFC Comments
    create_db!("ENGRAM/RFC Comments", rfc_comments, json!({
        "Comment": { "title": {} }, "Author": { "rich_text": {} },
        "Type": { "select": { "options": [
            { "name": "Concern", "color": "red" }, { "name": "Question", "color": "yellow" },
            { "name": "Approval", "color": "green" }, { "name": "Blocking", "color": "red" }
        ]}},
        "Resolved": { "checkbox": {} }, "Posted At": { "date": {} }
    }));

    // 4. Benchmarks
    create_db!("ENGRAM/Benchmarks", benchmarks, json!({
        "Benchmark ID": { "title": {} }, "Name": { "rich_text": {} },
        "Metric Type": { "select": { "options": [
            { "name": "Latency" }, { "name": "Throughput" }, { "name": "Memory" },
            { "name": "CPU" }, { "name": "Binary Size" }, { "name": "Startup Time" }
        ]}},
        "Value": { "number": { "format": "number" } }, "Unit": { "rich_text": {} },
        "Commit SHA": { "rich_text": {} }, "Branch": { "rich_text": {} },
        "Baseline Value": { "number": { "format": "number" } },
        "Delta %": { "number": { "format": "percent" } },
        "Status": { "select": { "options": [
            { "name": "Normal", "color": "green" }, { "name": "Warning", "color": "yellow" },
            { "name": "Regression", "color": "orange" }, { "name": "Critical", "color": "red" }
        ]}},
        "Tool": { "rich_text": {} }, "CI Run URL": { "url": {} }, "Timestamp": { "date": {} }
    }));

    // 5. Regressions
    create_db!("ENGRAM/Regressions", regressions, json!({
        "Regression ID": { "title": {} },
        "Severity": { "select": { "options": [
            { "name": "Warning", "color": "yellow" }, { "name": "Critical", "color": "red" },
            { "name": "Production Impact", "color": "red" }
        ]}},
        "Commit Range": { "rich_text": {} }, "Suspected Cause": { "rich_text": {} },
        "Bisect Command": { "rich_text": {} },
        "Status": { "select": { "options": [
            { "name": "Open", "color": "red" }, { "name": "Investigating", "color": "yellow" },
            { "name": "Root Cause Found", "color": "blue" }, { "name": "Resolved", "color": "green" },
            { "name": "Won't Fix", "color": "gray" }
        ]}},
        "Impact Assessment": { "rich_text": {} }, "Assigned To": { "rich_text": {} },
        "Opened At": { "date": {} }, "Resolved At": { "date": {} }
    }));

    // 6. Performance Baselines
    create_db!("ENGRAM/Performance Baselines", performance_baselines, json!({
        "Baseline ID": { "title": {} }, "Metric Name": { "rich_text": {} },
        "Rolling Mean": { "number": { "format": "number" } },
        "Rolling Stddev": { "number": { "format": "number" } },
        "Window Size": { "number": { "format": "number" } },
        "Warning Threshold %": { "number": { "format": "percent" } },
        "Critical Threshold %": { "number": { "format": "percent" } },
        "Last Updated": { "date": {} }
    }));

    // 7. Dependencies
    create_db!("ENGRAM/Dependencies", dependencies, json!({
        "Package ID": { "title": {} }, "Package Name": { "rich_text": {} },
        "Version": { "rich_text": {} },
        "Severity": { "select": { "options": [
            { "name": "Critical", "color": "red" }, { "name": "High", "color": "orange" },
            { "name": "Medium", "color": "yellow" }, { "name": "Low", "color": "blue" },
            { "name": "None", "color": "gray" }
        ]}},
        "CVE IDs": { "rich_text": {} }, "CVSS Score": { "number": { "format": "number" } },
        "Fix Available": { "checkbox": {} }, "Fixed In Version": { "rich_text": {} },
        "Triage Status": { "select": { "options": [
            { "name": "New", "color": "gray" }, { "name": "Accepted Risk", "color": "yellow" },
            { "name": "Fix Scheduled", "color": "blue" }, { "name": "Fixed", "color": "green" },
            { "name": "Won't Fix", "color": "red" }
        ]}},
        "AI Recommendation": { "rich_text": {} }, "Triage Reasoning": { "rich_text": {} },
        "First Seen": { "date": {} }, "Last Verified": { "date": {} }
    }));

    // 8. Audit Runs
    create_db!("ENGRAM/Audit Runs", audit_runs, json!({
        "Run Name": { "title": {} },
        "Tool": { "select": { "options": [
            { "name": "cargo-audit" }, { "name": "npm-audit" },
            { "name": "pip-audit" }, { "name": "osv-scanner" }
        ]}},
        "Findings Count": { "number": { "format": "number" } },
        "Critical Count": { "number": { "format": "number" } },
        "High Count": { "number": { "format": "number" } },
        "Commit SHA": { "rich_text": {} }, "Branch": { "rich_text": {} },
        "Project ID": { "rich_text": {} }
    }));

    // 9. Modules
    create_db!("ENGRAM/Modules", modules, json!({
        "Module Name": { "title": {} }, "Path": { "rich_text": {} },
        "Language": { "select": { "options": [
            { "name": "Rust" }, { "name": "TypeScript" }, { "name": "Python" },
            { "name": "Go" }, { "name": "Java" }, { "name": "Other" }
        ]}},
        "What It Does": { "rich_text": {} }, "Key Files": { "rich_text": {} },
        "Main Abstractions": { "rich_text": {} }, "Entry Points": { "rich_text": {} },
        "Common Gotchas": { "rich_text": {} }, "Owner": { "rich_text": {} },
        "Status": { "select": { "options": [
            { "name": "Active", "color": "green" }, { "name": "Deprecated", "color": "red" },
            { "name": "Experimental", "color": "yellow" }
        ]}},
        "Complexity Score": { "number": { "format": "number" } },
        "Test Coverage %": { "number": { "format": "percent" } }
    }));

    // 10–11. Onboarding
    create_db!("ENGRAM/Onboarding Tracks", onboarding_tracks, json!({
        "Track Name": { "title": {} },
        "Role": { "select": { "options": [
            { "name": "Backend" }, { "name": "Frontend" }, { "name": "DevOps" },
            { "name": "Full-Stack" }, { "name": "OSS Contributor" }
        ]}},
        "Estimated Hours": { "number": { "format": "number" } },
        "Step Count": { "number": { "format": "number" } }
    }));

    create_db!("ENGRAM/Onboarding Steps", onboarding_steps, json!({
        "Step Title": { "title": {} }, "Week/Day": { "rich_text": {} },
        "Type": { "select": { "options": [
            { "name": "Environment Setup" }, { "name": "Reading" },
            { "name": "Hands-on Task" }, { "name": "Code Exploration" }, { "name": "Quiz" }
        ]}},
        "Description": { "rich_text": {} },
        "Estimated Time": { "number": { "format": "number" } },
        "Verification": { "rich_text": {} }, "Auto-generated": { "checkbox": {} }
    }));

    // 12. Knowledge Gaps
    create_db!("ENGRAM/Knowledge Gaps", knowledge_gaps, json!({
        "Gap Title": { "title": {} },
        "Type": { "select": { "options": [
            { "name": "Undocumented Module" }, { "name": "Stale Doc" },
            { "name": "Missing Onboarding Step" }, { "name": "Orphaned RFC" }
        ]}},
        "Severity": { "select": { "options": [
            { "name": "Critical", "color": "red" }, { "name": "High", "color": "orange" },
            { "name": "Medium", "color": "yellow" }, { "name": "Low", "color": "blue" }
        ]}},
        "Status": { "select": { "options": [
            { "name": "Open", "color": "red" }, { "name": "In Progress", "color": "yellow" },
            { "name": "Resolved", "color": "green" }
        ]}},
        "Detected At": { "date": {} }
    }));

    // 13–15. Vault (Env Config, Config Snapshots, Secret Rotation Log)
    create_db!("ENGRAM/Env Config", env_config, json!({
        "Var Name": { "title": {} },
        "Environment": { "multi_select": { "options": [
            { "name": "dev" }, { "name": "staging" }, { "name": "prod" }, { "name": "ci" }
        ]}},
        "Type": { "select": { "options": [
            { "name": "API Key" }, { "name": "DB URL" }, { "name": "Feature Flag" },
            { "name": "Secret" }, { "name": "Config" }
        ]}},
        "Description": { "rich_text": {} },
        "Sensitivity": { "select": { "options": [
            { "name": "Public", "color": "green" }, { "name": "Internal", "color": "yellow" },
            { "name": "Secret", "color": "orange" }, { "name": "Critical Secret", "color": "red" }
        ]}},
        "Status": { "select": { "options": [
            { "name": "Active", "color": "green" }, { "name": "Deprecated", "color": "gray" },
            { "name": "Needs Rotation", "color": "orange" }
        ]}}
    }));

    create_db!("ENGRAM/Config Snapshots", config_snapshots, json!({
        "Snapshot ID": { "title": {} }, "Environment": { "rich_text": {} },
        "Total Vars": { "number": { "format": "number" } },
        "Missing Vars": { "number": { "format": "number" } },
        "AI Notes": { "rich_text": {} }, "Snapshot At": { "date": {} }
    }));

    create_db!("ENGRAM/Secret Rotation Log", secret_rotation_log, json!({
        "Log ID": { "title": {} }, "Rotated By": { "rich_text": {} },
        "Reason": { "select": { "options": [
            { "name": "Scheduled" }, { "name": "Breach Suspected" },
            { "name": "Engineer Offboarding" }, { "name": "Audit" }
        ]}},
        "Rotated At": { "date": {} }, "Notes": { "rich_text": {} }
    }));

    // 16–18. Review (PR Reviews, Review Playbook, Review Patterns)
    create_db!("ENGRAM/PR Reviews", pr_reviews, json!({
        "PR ID": { "title": {} }, "Title": { "rich_text": {} },
        "Author": { "rich_text": {} }, "Branch": { "rich_text": {} },
        "Status": { "select": { "options": [
            { "name": "Open", "color": "blue" }, { "name": "Changes Requested", "color": "orange" },
            { "name": "Approved", "color": "green" }, { "name": "Merged", "color": "purple" },
            { "name": "Closed", "color": "gray" }
        ]}},
        "Blocker Count": { "number": { "format": "number" } },
        "Suggestion Count": { "number": { "format": "number" } },
        "Claude Review Draft": { "rich_text": {} },
        "Opened At": { "date": {} }, "Merged At": { "date": {} }
    }));

    create_db!("ENGRAM/Review Playbook", review_playbook, json!({
        "Rule ID": { "title": {} }, "Title": { "rich_text": {} },
        "Category": { "select": { "options": [
            { "name": "Safety" }, { "name": "Performance" }, { "name": "Security" },
            { "name": "Architecture" }, { "name": "Testing" }, { "name": "Style" },
            { "name": "Documentation" }
        ]}},
        "Description": { "rich_text": {} }, "Good Example": { "rich_text": {} },
        "Bad Example": { "rich_text": {} },
        "Severity": { "select": { "options": [
            { "name": "Blocker", "color": "red" }, { "name": "Suggestion", "color": "yellow" },
            { "name": "Nit", "color": "blue" }
        ]}},
        "Active": { "checkbox": {} }, "Pattern Count": { "number": { "format": "number" } }
    }));

    create_db!("ENGRAM/Review Patterns", review_patterns, json!({
        "Pattern Name": { "title": {} }, "Category": { "rich_text": {} },
        "Frequency": { "number": { "format": "number" } },
        "Trend": { "select": { "options": [
            { "name": "Increasing", "color": "red" }, { "name": "Stable", "color": "yellow" },
            { "name": "Decreasing", "color": "green" }
        ]}},
        "First Seen": { "date": {} }, "Last Seen": { "date": {} },
        "AI Summary": { "rich_text": {} }
    }));

    // 19. Tech Debt
    create_db!("ENGRAM/Tech Debt", tech_debt, json!({
        "Debt Item": { "title": {} },
        "Source": { "select": { "options": [
            { "name": "Review Pattern" }, { "name": "RFC Drift" },
            { "name": "Stale Dependency" }, { "name": "Missing Tests" }
        ]}},
        "Severity": { "select": { "options": [
            { "name": "Critical", "color": "red" }, { "name": "High", "color": "orange" },
            { "name": "Medium", "color": "yellow" }, { "name": "Low", "color": "blue" }
        ]}},
        "Status": { "select": { "options": [
            { "name": "Identified", "color": "gray" }, { "name": "Triaged", "color": "yellow" },
            { "name": "Scheduled", "color": "blue" }, { "name": "Resolved", "color": "green" }
        ]}},
        "Identified At": { "date": {} }
    }));

    // 20–23. Cross-cutting (Health, Digest, Events, Releases)
    create_db!("ENGRAM/Health Reports", health_reports, json!({
        "Report ID": { "title": {} },
        "Period": { "select": { "options": [{ "name": "Weekly" }, { "name": "Monthly" }] }},
        "Decisions Health": { "number": { "format": "number" } },
        "Pulse Health": { "number": { "format": "number" } },
        "Shield Health": { "number": { "format": "number" } },
        "Atlas Health": { "number": { "format": "number" } },
        "Vault Health": { "number": { "format": "number" } },
        "Review Health": { "number": { "format": "number" } },
        "Overall Score": { "number": { "format": "number" } },
        "AI Narrative": { "rich_text": {} }, "Generated At": { "date": {} }
    }));

    create_db!("ENGRAM/Engineering Digest", engineering_digest, json!({
        "Digest ID": { "title": {} },
        "New RFCs": { "number": { "format": "number" } },
        "Regressions Found": { "number": { "format": "number" } },
        "New Vulnerabilities": { "number": { "format": "number" } },
        "PRs Reviewed": { "number": { "format": "number" } },
        "Health Score": { "number": { "format": "number" } },
        "Narrative": { "rich_text": {} }, "Action Items": { "rich_text": {} },
        "Generated At": { "date": {} }
    }));

    create_db!("ENGRAM/Events", events, json!({
        "Event ID": { "title": {} },
        "Type": { "select": { "options": [
            { "name": "RFC Created" }, { "name": "RFC Approved" },
            { "name": "Regression Detected" }, { "name": "CVE Found" },
            { "name": "PR Merged" }, { "name": "Secret Rotated" },
            { "name": "New Engineer" }, { "name": "Module Updated" },
            { "name": "Debt Created" }, { "name": "Health Report" }
        ]}},
        "Title": { "rich_text": {} },
        "Severity": { "select": { "options": [
            { "name": "Info", "color": "blue" }, { "name": "Warning", "color": "yellow" },
            { "name": "Critical", "color": "red" }
        ]}},
        "Source Layer": { "select": { "options": [
            { "name": "Decisions" }, { "name": "Pulse" }, { "name": "Shield" },
            { "name": "Atlas" }, { "name": "Vault" }, { "name": "Review" }
        ]}},
        "Occurred At": { "date": {} }
    }));

    create_db!("ENGRAM/Releases", releases, json!({
        "Release ID": { "title": {} },
        "Status": { "select": { "options": [
            { "name": "Draft", "color": "gray" }, { "name": "Candidate", "color": "yellow" },
            { "name": "Released", "color": "green" }
        ]}},
        "Milestone": { "rich_text": {} },
        "Regression Free": { "checkbox": {} }, "CVE Free": { "checkbox": {} },
        "Release Notes": { "rich_text": {} }, "Released At": { "date": {} }
    }));

    Ok(db)
}

/// Add cross-database relation fields between databases.
pub async fn create_relations(notion: &NotionMcpClient, db: &DatabaseIds) -> Result<()> {
    info!("[Setup] Adding cross-database relation fields...");

    let projects_id = &db.projects;

    // Project relation for 12 databases
    let targets: Vec<(&str, &str)> = vec![
        (&db.rfcs, "RFCs"), (&db.benchmarks, "Benchmarks"),
        (&db.regressions, "Regressions"), (&db.dependencies, "Dependencies"),
        (&db.modules, "Modules"), (&db.pr_reviews, "PR Reviews"),
        (&db.health_reports, "Health Reports"), (&db.events, "Events"),
        (&db.releases, "Releases"), (&db.env_config, "Env Config"),
        (&db.tech_debt, "Tech Debt"), (&db.engineering_digest, "Eng Digest"),
    ];

    for (db_id, name) in &targets {
        let _ = notion.patch(
            &format!("/databases/{db_id}"),
            &json!({ "properties": { "Project": { "relation": { "database_id": projects_id, "single_property": {} } } } }),
        ).await;
        info!("[Setup] {name} → Project relation added");
    }

    // RFC Comments → RFC
    let _ = notion.patch(&format!("/databases/{}", db.rfc_comments),
        &json!({ "properties": { "RFC": { "relation": { "database_id": db.rfcs, "single_property": {} } } } })).await;
    // Regressions → Benchmarks
    let _ = notion.patch(&format!("/databases/{}", db.regressions),
        &json!({ "properties": { "Source Benchmark": { "relation": { "database_id": db.benchmarks, "single_property": {} } } } })).await;
    // PR Reviews → RFCs
    let _ = notion.patch(&format!("/databases/{}", db.pr_reviews),
        &json!({ "properties": { "Related RFCs": { "relation": { "database_id": db.rfcs, "single_property": {} } } } })).await;
    // Tech Debt → PR Reviews
    let _ = notion.patch(&format!("/databases/{}", db.tech_debt),
        &json!({ "properties": { "Source PR": { "relation": { "database_id": db.pr_reviews, "single_property": {} } } } })).await;
    // Releases → RFCs
    let _ = notion.patch(&format!("/databases/{}", db.releases),
        &json!({ "properties": { "Implemented RFCs": { "relation": { "database_id": db.rfcs, "single_property": {} } } } })).await;

    info!("[Setup] All 18 cross-database relations created");
    Ok(())
}

/// Create default playbook rules.
pub async fn create_default_playbook(notion: &NotionMcpClient, playbook_db_id: &str) -> Result<()> {
    info!("[Setup] Creating default review playbook rules...");

    let rules = vec![
        ("RULE-001", "Avoid .unwrap() in library code", "Safety", "Blocker"),
        ("RULE-002", "All public APIs must have doc comments", "Documentation", "Suggestion"),
        ("RULE-003", "No blocking calls in async context", "Performance", "Blocker"),
    ];

    for (id, title, category, severity) in rules {
        notion.create_page(playbook_db_id, json!({
            "Rule ID": { "title": [{ "text": { "content": id } }] },
            "Title": { "rich_text": [{ "text": { "content": title } }] },
            "Category": { "select": { "name": category } },
            "Severity": { "select": { "name": severity } },
            "Active": { "checkbox": true },
            "Pattern Count": { "number": 0 },
        })).await?;
    }

    info!("[Setup] 3 playbook rules created");
    Ok(())
}

/// Create a sample anchor project.
pub async fn create_sample_project(notion: &NotionMcpClient, projects_db_id: &str) -> Result<()> {
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    notion.create_page(projects_db_id, json!({
        "Name": { "title": [{ "text": { "content": "ENGRAM Project" } }] },
        "Description": { "rich_text": [{ "text": { "content": "Main project managed by ENGRAM" } }] },
        "Status": { "select": { "name": "Active" } },
        "Created At": { "date": { "start": today } },
    })).await?;
    info!("[Setup] Sample project created");
    Ok(())
}

/// Write database IDs back to engram.toml.
pub fn persist_database_ids(
    config_path: &std::path::Path,
    db_ids: &DatabaseIds,
) -> Result<()> {
    let content = std::fs::read_to_string(config_path)?;
    let mut doc: toml::Table = content.parse()?;

    let dbs = toml::Table::from_iter([
        ("projects".into(), toml::Value::String(db_ids.projects.clone())),
        ("rfcs".into(), toml::Value::String(db_ids.rfcs.clone())),
        ("rfc_comments".into(), toml::Value::String(db_ids.rfc_comments.clone())),
        ("benchmarks".into(), toml::Value::String(db_ids.benchmarks.clone())),
        ("regressions".into(), toml::Value::String(db_ids.regressions.clone())),
        ("performance_baselines".into(), toml::Value::String(db_ids.performance_baselines.clone())),
        ("dependencies".into(), toml::Value::String(db_ids.dependencies.clone())),
        ("audit_runs".into(), toml::Value::String(db_ids.audit_runs.clone())),
        ("modules".into(), toml::Value::String(db_ids.modules.clone())),
        ("onboarding_tracks".into(), toml::Value::String(db_ids.onboarding_tracks.clone())),
        ("onboarding_steps".into(), toml::Value::String(db_ids.onboarding_steps.clone())),
        ("knowledge_gaps".into(), toml::Value::String(db_ids.knowledge_gaps.clone())),
        ("env_config".into(), toml::Value::String(db_ids.env_config.clone())),
        ("config_snapshots".into(), toml::Value::String(db_ids.config_snapshots.clone())),
        ("secret_rotation_log".into(), toml::Value::String(db_ids.secret_rotation_log.clone())),
        ("pr_reviews".into(), toml::Value::String(db_ids.pr_reviews.clone())),
        ("review_playbook".into(), toml::Value::String(db_ids.review_playbook.clone())),
        ("review_patterns".into(), toml::Value::String(db_ids.review_patterns.clone())),
        ("tech_debt".into(), toml::Value::String(db_ids.tech_debt.clone())),
        ("health_reports".into(), toml::Value::String(db_ids.health_reports.clone())),
        ("engineering_digest".into(), toml::Value::String(db_ids.engineering_digest.clone())),
        ("events".into(), toml::Value::String(db_ids.events.clone())),
        ("releases".into(), toml::Value::String(db_ids.releases.clone())),
    ]);

    doc.insert("databases".to_string(), toml::Value::Table(dbs));
    std::fs::write(config_path, toml::to_string_pretty(&doc)?)?;
    info!("[Setup] Database IDs written to {}", config_path.display());
    Ok(())
}

fn extract_id(result: &serde_json::Value) -> String {
    result["id"]
        .as_str()
        .or_else(|| result["data"]["id"].as_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Find an existing "ENGRAM" page in the workspace, or create one at the workspace root.
/// No parent page ID needed — creates a top-level page automatically.
async fn find_or_create_engram_page(
    notion: &NotionMcpClient,
) -> Result<String> {
    // First, try to search for an existing ENGRAM page
    info!("[Setup] Searching for existing ENGRAM parent page...");
    if let Ok(search_result) = notion.search("ENGRAM", Some(json!({
        "value": "page",
        "property": "object"
    }))).await {
        if let Some(results) = search_result["results"].as_array() {
            for page in results {
                let title = page["properties"]["title"]["title"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|t| t["plain_text"].as_str())
                    .unwrap_or("");
                if title == "ENGRAM" && page["archived"].as_bool() != Some(true) {
                    let id = page["id"].as_str().unwrap_or("").to_string();
                    if !id.is_empty() {
                        info!("[Setup] Found existing ENGRAM page: {}", id);
                        return Ok(id);
                    }
                }
            }
        }
    }

    // No existing page found — create one at workspace root with rich styling
    info!("[Setup] Creating new ENGRAM parent page at workspace root...");
    let page_payload = json!({
        "parent": { "type": "workspace", "workspace": true },
        "properties": {
            "title": {
                "title": [{
                    "text": { "content": "ENGRAM" },
                    "annotations": { "bold": true }
                }]
            }
        },
        "icon": { "type": "emoji", "emoji": "🧠" },
        "cover": {
            "type": "external",
            "external": { "url": "https://raw.githubusercontent.com/manojpisini/engram/main/images/engram_banner.png" }
        },
        "children": [
            {
                "object": "block",
                "type": "callout",
                "callout": {
                    "icon": { "type": "emoji", "emoji": "⚡" },
                    "color": "yellow_background",
                    "rich_text": [{
                        "type": "text",
                        "text": { "content": "This page is managed by ENGRAM — Engineering Intelligence Platform. All databases below are auto-created and continuously updated by 9 AI-powered analysis agents." },
                        "annotations": { "color": "default" }
                    }]
                }
            },
            {
                "object": "block",
                "type": "divider",
                "divider": {}
            },
            {
                "object": "block",
                "type": "heading_2",
                "heading_2": {
                    "rich_text": [{
                        "type": "text",
                        "text": { "content": "🧭 Intelligence Layers" },
                        "annotations": { "color": "default" }
                    }]
                }
            },
            {
                "object": "block",
                "type": "column_list",
                "column_list": {
                    "children": [
                        {
                            "object": "block",
                            "type": "column",
                            "column": {
                                "children": [
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "🧭 Decisions" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — RFCs, decision drift, stale proposals" } }]
                                        }
                                    },
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "📊 Pulse" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — Benchmarks, regressions, baselines" } }]
                                        }
                                    },
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "🛡️ Shield" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — Dependencies, CVE audits" } }]
                                        }
                                    },
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "🗺️ Atlas" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — Module maps, onboarding tracks" } }]
                                        }
                                    },
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "🔐 Vault" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — Env configs, secret rotation" } }]
                                        }
                                    }
                                ]
                            }
                        },
                        {
                            "object": "block",
                            "type": "column",
                            "column": {
                                "children": [
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "📝 Review" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — PR patterns, tech debt tracking" } }]
                                        }
                                    },
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "❤️ Health" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — Repo health scores, digests" } }]
                                        }
                                    },
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "⏱️ Timeline" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — Milestones, velocity tracking" } }]
                                        }
                                    },
                                    {
                                        "object": "block",
                                        "type": "bulleted_list_item",
                                        "bulleted_list_item": {
                                            "rich_text": [{ "type": "text", "text": { "content": "🚀 Release" }, "annotations": { "bold": true } }, { "type": "text", "text": { "content": " — Auto release notes, changelogs" } }]
                                        }
                                    }
                                ]
                            }
                        }
                    ]
                }
            },
            {
                "object": "block",
                "type": "divider",
                "divider": {}
            },
            {
                "object": "block",
                "type": "heading_2",
                "heading_2": {
                    "rich_text": [{
                        "type": "text",
                        "text": { "content": "📚 Databases" }
                    }]
                }
            },
            {
                "object": "block",
                "type": "paragraph",
                "paragraph": {
                    "rich_text": [{
                        "type": "text",
                        "text": { "content": "All 23 databases are created below automatically. Each database is populated in real-time by the corresponding intelligence agent when GitHub webhook events arrive." },
                        "annotations": { "color": "gray" }
                    }]
                }
            },
            {
                "object": "block",
                "type": "divider",
                "divider": {}
            }
        ]
    });

    let result = notion.post_raw("/pages", &page_payload).await
        .context("Failed to create ENGRAM parent page at workspace root. Ensure the integration token is valid and has 'Insert content' capability enabled.")?;

    let page_id = extract_id(&result);
    if page_id == "unknown" || page_id.is_empty() {
        anyhow::bail!("Created ENGRAM page but got no ID in response");
    }

    info!("[Setup] Created ENGRAM parent page: {}", page_id);
    Ok(page_id)
}

/// Returns the ENGRAM parent page ID (for API responses).
/// Searches for the existing page without creating one.
pub async fn get_engram_page_id(notion: &NotionMcpClient) -> Option<String> {
    if let Ok(search_result) = notion.search("ENGRAM", Some(json!({
        "value": "page",
        "property": "object"
    }))).await {
        if let Some(results) = search_result["results"].as_array() {
            for page in results {
                let title = page["properties"]["title"]["title"]
                    .as_array()
                    .and_then(|a| a.first())
                    .and_then(|t| t["plain_text"].as_str())
                    .unwrap_or("");
                if title == "ENGRAM" && page["archived"].as_bool() != Some(true) {
                    return page["id"].as_str().map(|s| s.to_string());
                }
            }
        }
    }
    None
}
