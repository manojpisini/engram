use std::sync::Arc;

use anyhow::Result;
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json,
};
use hmac::{Hmac, Mac};
use serde::Deserialize;
use sha2::Sha256;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{error, info, warn};

use engram_types::events::{AuditTool, EngramEvent};

use crate::AppState;
use crate::notion_client::NotionMcpClient;

type HmacSha256 = Hmac<Sha256>;

/// Start the axum webhook listener
pub async fn serve(state: Arc<AppState>, addr: &str) -> Result<()> {
    // Resolve the dashboard directory relative to the current working directory
    let dashboard_path = std::env::current_dir()
        .unwrap_or_default()
        .join("dashboard");

    let app = Router::new()
        .route("/health", get(health_check))
        .route("/webhook/github", post(github_webhook))
        .route("/webhook/benchmark", post(benchmark_webhook))
        .route("/webhook/audit", post(audit_webhook))
        .route("/api/trigger/digest", post(trigger_digest))
        .route("/api/trigger/onboard", post(trigger_onboard))
        .route("/api/trigger/release", post(trigger_release))
        .route("/api/projects", get(api_projects))
        .route("/api/dashboard/health", get(api_dashboard_health))
        .route("/api/dashboard/events", get(api_dashboard_events))
        .route("/api/dashboard/rfcs", get(api_dashboard_rfcs))
        .route("/api/dashboard/vulnerabilities", get(api_dashboard_vulns))
        .route("/api/dashboard/reviews", get(api_dashboard_reviews))
        .route("/api/dashboard/modules", get(api_dashboard_modules))
        .route("/api/dashboard/env-config", get(api_dashboard_env_config))
        .route("/api/dashboard/benchmarks", get(api_dashboard_benchmarks))
        .route("/api/dashboard/releases", get(api_dashboard_releases))
        .route("/api/github/repos", get(api_github_repos))
        .route("/api/github/repo/:owner/:repo", get(api_github_repo_detail))
        .route("/api/github/repo/:owner/:repo/pulls", get(api_github_pulls))
        .route("/api/github/repo/:owner/:repo/issues", get(api_github_issues))
        .route("/api/github/repo/:owner/:repo/commits", get(api_github_commits))
        .route("/api/github/repo/:owner/:repo/contributors", get(api_github_contributors))
        .route("/api/github/connection", get(api_github_connection))
        .route("/api/notion/connection", get(api_notion_connection))
        .route("/api/notion/search", get(api_notion_search))
        .route("/api/notion/database/:db_id", get(api_notion_database_schema))
        .route("/api/dashboard/tech-debt", get(api_dashboard_tech_debt))
        .route("/api/dashboard/digest", get(api_dashboard_digest))
        .route("/api/config", get(api_get_config))
        .route("/api/config/update", post(api_update_config))
        .route("/api/setup/notion", post(api_setup_notion))
        .route("/api/setup/status", get(api_setup_status))
        .route("/api/intelligence/generate", post(api_intelligence_generate))
        .fallback_service(ServeDir::new(&dashboard_path))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    info!("Serving dashboard from {}", dashboard_path.display());

    // Try to bind; if port is in use, attempt to kill the old process (Windows)
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(l) => l,
        Err(e) if e.kind() == std::io::ErrorKind::AddrInUse => {
            warn!("Port {} already in use — attempting to free it...", addr);
            kill_old_process();
            // Brief pause for OS to release the port
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            tokio::net::TcpListener::bind(addr).await
                .map_err(|e2| anyhow::anyhow!("Port still in use after cleanup: {e2}"))?
        }
        Err(e) => return Err(e.into()),
    };

    info!("ENGRAM webhook listener running on {addr}");
    axum::serve(listener, app).await?;

    Ok(())
}

/// Try to kill any existing engram.exe process (Windows) or engram process (Unix)
fn kill_old_process() {
    #[cfg(target_os = "windows")]
    {
        // Use taskkill to stop any other engram.exe instances
        let current_pid = std::process::id();
        match std::process::Command::new("taskkill")
            .args(["/F", "/IM", "engram.exe"])
            .output()
        {
            Ok(output) => {
                let msg = String::from_utf8_lossy(&output.stdout);
                info!("taskkill output: {}", msg.trim());
            }
            Err(e) => {
                warn!("Failed to run taskkill: {e}");
            }
        }
        let _ = current_pid; // suppress unused warning
    }
    #[cfg(not(target_os = "windows"))]
    {
        // On Unix, try to find and kill old engram process on the port
        warn!("Port in use — please stop the existing engram process manually");
    }
}

// ─── Helper: read config/notion from RwLock ───

/// Clone the config from the RwLock (fast — just strings)
fn cfg(state: &AppState) -> engram_types::config::EngramConfig {
    state.config.read().unwrap().clone()
}

/// Clone the Notion client from the RwLock (reqwest::Client is Arc-based, cheap)
fn notion(state: &AppState) -> NotionMcpClient {
    state.notion.read().unwrap().clone()
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "engram-core",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ─── GitHub Webhook ───

#[derive(Debug, Deserialize)]
struct GitHubWebhookPayload {
    action: Option<String>,
    pull_request: Option<PullRequestPayload>,
    repository: Option<RepoPayload>,
}

#[derive(Debug, Deserialize)]
struct PullRequestPayload {
    number: u64,
    title: String,
    body: Option<String>,
    diff_url: Option<String>,
    user: UserPayload,
    head: BranchPayload,
    base: BranchPayload,
    merged: Option<bool>,
    merge_commit_sha: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserPayload {
    login: String,
}

#[derive(Debug, Deserialize)]
struct BranchPayload {
    #[serde(rename = "ref")]
    ref_name: String,
}

#[derive(Debug, Deserialize)]
struct RepoPayload {
    full_name: String,
}

async fn github_webhook(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let secret = cfg(&state).auth.webhook_secret;
    // Verify webhook signature
    if let Err(e) = verify_github_signature(&headers, &body, &secret) {
        warn!("GitHub webhook signature verification failed: {e}");
        return StatusCode::UNAUTHORIZED;
    }

    let payload: GitHubWebhookPayload = match serde_json::from_str(&body) {
        Ok(p) => p,
        Err(e) => {
            error!("Failed to parse GitHub webhook: {e}");
            return StatusCode::BAD_REQUEST;
        }
    };

    let event_type = headers
        .get("x-github-event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");

    match event_type {
        "pull_request" => handle_pr_event(&state, &payload),
        _ => {
            info!("Ignoring GitHub event type: {event_type}");
        }
    }

    StatusCode::OK
}

fn handle_pr_event(state: &Arc<AppState>, payload: &GitHubWebhookPayload) {
    let Some(pr) = &payload.pull_request else { return };
    let Some(repo) = &payload.repository else { return };
    let action = payload.action.as_deref().unwrap_or("");

    let body_text = pr.body.clone().unwrap_or_default();

    // Extract RFC references from PR body (e.g., "RFC-0001", "Implements RFC-0042")
    let rfc_refs: Vec<String> = regex::Regex::new(r"RFC-\d{4}")
        .unwrap()
        .find_iter(&body_text)
        .map(|m| m.as_str().to_string())
        .collect();

    match action {
        "opened" | "synchronize" | "reopened" => {
            let event = EngramEvent::PrOpened {
                repo: repo.full_name.clone(),
                pr_number: pr.number,
                diff: pr.diff_url.clone().unwrap_or_default(),
                title: pr.title.clone(),
                description: body_text,
                author: pr.user.login.clone(),
                branch: pr.head.ref_name.clone(),
                target_branch: pr.base.ref_name.clone(),
            };
            state.router.dispatch(event);
        }
        "closed" if pr.merged == Some(true) => {
            let event = EngramEvent::PrMerged {
                repo: repo.full_name.clone(),
                pr_number: pr.number,
                diff: pr.diff_url.clone().unwrap_or_default(),
                branch: pr.head.ref_name.clone(),
                commit_sha: pr.merge_commit_sha.clone().unwrap_or_default(),
                title: pr.title.clone(),
                author: pr.user.login.clone(),
                rfc_references: rfc_refs,
            };
            state.router.dispatch(event);
        }
        _ => {
            info!("Ignoring PR action: {action}");
        }
    }
}

fn verify_github_signature(headers: &HeaderMap, body: &str, secret: &str) -> Result<()> {
    // If no webhook secret is configured, skip signature verification (first-install scenario).
    // The user can set a secret later from dashboard Settings for production use.
    if secret.is_empty() {
        return Ok(());
    }

    let sig_header = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| anyhow::anyhow!("Missing X-Hub-Signature-256 header"))?;

    let sig_hex = sig_header
        .strip_prefix("sha256=")
        .ok_or_else(|| anyhow::anyhow!("Invalid signature format"))?;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(body.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());

    if expected != sig_hex {
        anyhow::bail!("Signature mismatch");
    }

    Ok(())
}

// ─── Benchmark Webhook (from CI) ───

#[derive(Debug, Deserialize)]
struct BenchmarkPayload {
    project_id: String,
    commit_sha: String,
    branch: String,
    benchmarks: serde_json::Value,
}

async fn benchmark_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<BenchmarkPayload>,
) -> impl IntoResponse {
    info!(
        "Received benchmark results for project {} (commit: {})",
        payload.project_id,
        &payload.commit_sha[..8.min(payload.commit_sha.len())]
    );

    let event = EngramEvent::CiBenchmarkPosted {
        project_id: payload.project_id,
        raw_json: payload.benchmarks.to_string(),
        commit_sha: payload.commit_sha,
        branch: payload.branch,
    };
    state.router.dispatch(event);

    StatusCode::OK
}

// ─── Audit Webhook (from CI) ───

#[derive(Debug, Deserialize)]
struct AuditPayload {
    project_id: String,
    raw_output: String,
    tool: String,
    commit_sha: String,
    branch: Option<String>,
}

async fn audit_webhook(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<AuditPayload>,
) -> impl IntoResponse {
    let tool = match payload.tool.as_str() {
        "cargo-audit" => AuditTool::CargoAudit,
        "npm-audit" => AuditTool::NpmAudit,
        "pip-audit" => AuditTool::PipAudit,
        "osv-scanner" => AuditTool::OsvScanner,
        other => {
            error!("Unknown audit tool: {other}");
            return StatusCode::BAD_REQUEST;
        }
    };

    info!("Received audit results ({}) for project {}", tool, payload.project_id);

    let event = EngramEvent::CiAuditPosted {
        project_id: payload.project_id,
        raw_output: payload.raw_output,
        tool,
        commit_sha: payload.commit_sha,
        branch: payload.branch.unwrap_or_else(|| "main".to_string()),
    };
    state.router.dispatch(event);

    StatusCode::OK
}

// ─── Manual Triggers ───

#[derive(Debug, Deserialize)]
struct TriggerPayload {
    project_id: String,
}

async fn trigger_digest(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<TriggerPayload>,
) -> impl IntoResponse {
    info!("Manual digest trigger for project {}", payload.project_id);
    state.router.dispatch(EngramEvent::WeeklyDigestTrigger {
        project_id: payload.project_id,
    });
    StatusCode::OK
}

#[derive(Debug, Deserialize)]
struct OnboardPayload {
    engineer_name: String,
    role: String,
    project_id: String,
    #[serde(default)]
    repo: String,
}

async fn trigger_onboard(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<OnboardPayload>,
) -> impl IntoResponse {
    use engram_types::events::Role;

    let role = match payload.role.to_lowercase().as_str() {
        "backend" => Role::Backend,
        "frontend" => Role::Frontend,
        "devops" => Role::DevOps,
        "full-stack" | "fullstack" => Role::FullStack,
        "oss" | "oss-contributor" => Role::OssContributor,
        _ => {
            error!("Unknown role: {}", payload.role);
            return StatusCode::BAD_REQUEST;
        }
    };

    let repo = if payload.repo.is_empty() {
        // Default to first configured repo
        cfg(&state).github.repos.first().cloned().unwrap_or_default()
    } else {
        payload.repo
    };

    info!("New engineer onboard: {} ({}) for repo {}", payload.engineer_name, role, repo);
    state.router.dispatch(EngramEvent::NewEngineerOnboards {
        engineer_name: payload.engineer_name,
        role,
        project_id: payload.project_id,
        repo,
    });
    StatusCode::OK
}

#[derive(Debug, Deserialize)]
struct ReleasePayload {
    project_id: String,
    version: String,
    milestone: String,
}

async fn trigger_release(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ReleasePayload>,
) -> impl IntoResponse {
    info!("Release trigger: {} v{}", payload.project_id, payload.version);
    state.router.dispatch(EngramEvent::ReleaseCreated {
        project_id: payload.project_id,
        version: payload.version,
        milestone: payload.milestone,
    });
    StatusCode::OK
}

// ═══════════════════════════════════════════════════════════════
//  Dashboard API — serves data from Notion to the frontend
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct ProjectQuery {
    project_id: Option<String>,
}

/// GET /api/projects — list all projects
async fn api_projects(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let db_id = cfg(&state).databases.projects.clone();
    let n = notion(&state);
    match n.query_database(&db_id, None, None, Some(100), None).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => {
            error!("Failed to query projects: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// Helper: build project filter for a given project_id (or return None for all)
fn dash_project_filter(project_id: &Option<String>) -> Option<serde_json::Value> {
    project_id.as_ref().map(|pid| {
        serde_json::json!({
            "property": "Project ID",
            "rich_text": { "equals": pid }
        })
    })
}

/// Macro to reduce boilerplate for dashboard query endpoints.
/// Reads config and notion client from RwLock, then queries Notion.
macro_rules! dashboard_query {
    ($state:expr, $db_field:ident, $filter:expr, $sorts:expr, $limit:expr) => {{
        let db_id = cfg(&$state).databases.$db_field.clone();
        let n = notion(&$state);
        match n.query_database(&db_id, $filter, $sorts, Some($limit), None).await {
            Ok(resp) => Json(resp).into_response(),
            Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }};
}

/// Sort helper: newest first by created_time
fn sort_newest() -> Option<Vec<crate::notion_client::QuerySort>> {
    Some(vec![crate::notion_client::QuerySort {
        property: "created_time".to_string(),
        direction: crate::notion_client::SortDirection::Descending,
    }])
}

async fn api_dashboard_health(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, health_reports, dash_project_filter(&q.project_id), sort_newest(), 10)
}

async fn api_dashboard_events(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, events, dash_project_filter(&q.project_id), sort_newest(), 50)
}

async fn api_dashboard_rfcs(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, rfcs, dash_project_filter(&q.project_id), sort_newest(), 50)
}

async fn api_dashboard_vulns(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, dependencies, dash_project_filter(&q.project_id), sort_newest(), 50)
}

async fn api_dashboard_reviews(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, pr_reviews, dash_project_filter(&q.project_id), sort_newest(), 50)
}

async fn api_dashboard_modules(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, modules, dash_project_filter(&q.project_id), None, 100)
}

async fn api_dashboard_env_config(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, env_config, dash_project_filter(&q.project_id), None, 100)
}

async fn api_dashboard_benchmarks(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, benchmarks, dash_project_filter(&q.project_id), sort_newest(), 50)
}

async fn api_dashboard_releases(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, releases, dash_project_filter(&q.project_id), sort_newest(), 20)
}

async fn api_dashboard_tech_debt(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, tech_debt, dash_project_filter(&q.project_id), sort_newest(), 50)
}

async fn api_dashboard_digest(State(state): State<Arc<AppState>>, axum::extract::Query(q): axum::extract::Query<ProjectQuery>) -> impl IntoResponse {
    dashboard_query!(state, engineering_digest, dash_project_filter(&q.project_id), sort_newest(), 10)
}

// ─── Notion Search & Schema ───

#[derive(Debug, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

async fn api_notion_search(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(q): axum::extract::Query<SearchQuery>,
) -> impl IntoResponse {
    let query = q.q.as_deref().unwrap_or("");
    let n = notion(&state);
    match n.search(query, Some(serde_json::json!({"property": "object", "value": "page"}))).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
    }
}

async fn api_notion_database_schema(
    State(state): State<Arc<AppState>>,
    Path(db_id): Path<String>,
) -> impl IntoResponse {
    let n = notion(&state);
    match n.retrieve_database(&db_id).await {
        Ok(resp) => Json(resp).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
    }
}

// ═══════════════════════════════════════════════════════════════
//  GitHub & Notion API Proxy — serves GitHub data to the frontend
// ═══════════════════════════════════════════════════════════════

/// Helper: build a reqwest client with GitHub API headers
fn github_client(token: &str) -> Result<reqwest::Client, reqwest::Error> {
    use reqwest::header::{HeaderMap as ReqHeaderMap, HeaderValue};
    let mut headers = ReqHeaderMap::new();
    headers.insert("Accept", HeaderValue::from_static("application/vnd.github+json"));
    headers.insert("X-GitHub-Api-Version", HeaderValue::from_static("2022-11-28"));
    headers.insert("User-Agent", HeaderValue::from_static("ENGRAM/1.0"));
    if !token.is_empty() {
        if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token)) {
            headers.insert("Authorization", val);
        }
    }
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
}

/// GET /api/github/connection — check if GitHub token is configured and working
async fn api_github_connection(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let c = cfg(&state);
    let connected = !c.auth.github_token.is_empty();
    let repos = c.github.repos.clone();
    Json(serde_json::json!({
        "connected": connected,
        "repos": repos,
    }))
}

/// GET /api/notion/connection — check if Notion token is configured
async fn api_notion_connection(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let c = cfg(&state);
    let connected = !c.auth.notion_mcp_token.is_empty();
    let workspace_id = c.workspace.notion_workspace_id.clone();
    Json(serde_json::json!({
        "connected": connected,
        "workspace_id": workspace_id,
    }))
}

/// GET /api/config — return sanitized config (no secrets)
async fn api_get_config(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let c = cfg(&state);
    let db = &c.databases;

    // Count how many database IDs are configured (non-empty)
    let db_count = [
        &db.projects, &db.rfcs, &db.rfc_comments, &db.benchmarks,
        &db.regressions, &db.performance_baselines, &db.dependencies,
        &db.audit_runs, &db.modules, &db.onboarding_tracks,
        &db.onboarding_steps, &db.knowledge_gaps, &db.env_config,
        &db.config_snapshots, &db.secret_rotation_log, &db.pr_reviews,
        &db.review_playbook, &db.review_patterns, &db.tech_debt,
        &db.health_reports, &db.engineering_digest, &db.events, &db.releases,
    ]
    .iter()
    .filter(|id| !id.is_empty())
    .count();

    Json(serde_json::json!({
        "server": {
            "host": c.server.host,
            "port": c.server.port,
        },
        "github": {
            "repos": c.github.repos,
            "webhook_endpoint": "/webhook/github",
        },
        "schedule": {
            "daily_audit": c.schedule.daily_audit,
            "weekly_digest": c.schedule.weekly_digest,
            "weekly_rfc_staleness": c.schedule.weekly_rfc_staleness,
            "daily_rotation_check": c.schedule.daily_rotation_check,
            "weekly_knowledge_gap_scan": c.schedule.weekly_knowledge_gap_scan,
        },
        "thresholds": {
            "warning_delta_pct": c.thresholds.warning_delta_pct,
            "critical_delta_pct": c.thresholds.critical_delta_pct,
            "production_impact_delta_pct": c.thresholds.production_impact_delta_pct,
            "baseline_window": c.thresholds.baseline_window,
            "pattern_debt_threshold": c.thresholds.pattern_debt_threshold,
            "auto_rfc_severities": c.thresholds.auto_rfc_severities,
            "rfc_stale_days": c.thresholds.rfc_stale_days,
        },
        "claude": {
            "model": c.claude.model,
        },
        "databases": {
            "configured_count": db_count,
            "total": 23,
        },
        "auth": {
            "github_configured": !c.auth.github_token.is_empty(),
            "notion_configured": !c.auth.notion_mcp_token.is_empty(),
            "anthropic_configured": !c.auth.anthropic_api_key.is_empty(),
        },
    }))
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/config/update — hot-reload config from dashboard
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Deserialize)]
struct ConfigUpdatePayload {
    github_token: Option<String>,
    notion_token: Option<String>,
    anthropic_api_key: Option<String>,
    github_repos: Option<Vec<String>>,
    workspace_id: Option<String>,
    webhook_secret: Option<String>,
    server_host: Option<String>,
    server_port: Option<u16>,
}

/// POST /api/config/update — update tokens/repos at runtime and persist to engram.toml
async fn api_update_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConfigUpdatePayload>,
) -> impl IntoResponse {
    info!("Config update request received");

    // Read current config, apply updates
    let mut new_config = cfg(&state);

    if let Some(ref token) = payload.github_token {
        new_config.auth.github_token = token.clone();
    }
    if let Some(ref token) = payload.notion_token {
        new_config.auth.notion_mcp_token = token.clone();
    }
    if let Some(ref key) = payload.anthropic_api_key {
        new_config.auth.anthropic_api_key = key.clone();
    }
    if let Some(ref repos) = payload.github_repos {
        new_config.github.repos = repos.clone();
    }
    if let Some(ref wid) = payload.workspace_id {
        new_config.workspace.notion_workspace_id = wid.clone();
    }
    if let Some(ref secret) = payload.webhook_secret {
        new_config.auth.webhook_secret = secret.clone();
    }
    if let Some(ref host) = payload.server_host {
        new_config.server.host = host.clone();
    }
    if let Some(port) = payload.server_port {
        new_config.server.port = port;
    }

    // Persist to engram.toml
    let config_path = &state.config_path;
    if let Err(e) = persist_config_updates(config_path, &payload) {
        error!("Failed to persist config to {}: {e}", config_path.display());
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "error": format!("Failed to save config: {e}")
        }))).into_response();
    }

    // Rebuild Notion client if token changed
    let notion_changed = payload.notion_token.is_some() || payload.workspace_id.is_some();
    if notion_changed {
        let new_notion = NotionMcpClient::new(&new_config);
        *state.notion.write().unwrap() = new_notion;
    }

    // Update in-memory config
    *state.config.write().unwrap() = new_config;

    info!("Config updated successfully (in-memory + engram.toml)");
    Json(serde_json::json!({
        "status": "ok",
        "message": "Configuration updated. Changes are live immediately.",
    })).into_response()
}

/// Persist config updates to engram.toml using targeted line replacement.
/// Preserves comments and structure.
fn persist_config_updates(
    path: &std::path::Path,
    payload: &ConfigUpdatePayload,
) -> Result<()> {
    let mut content = std::fs::read_to_string(path)?;

    if let Some(ref token) = payload.github_token {
        content = replace_toml_value(&content, "github_token", &format!("\"{}\"", token));
    }
    if let Some(ref token) = payload.notion_token {
        content = replace_toml_value(&content, "notion_mcp_token", &format!("\"{}\"", token));
    }
    if let Some(ref key) = payload.anthropic_api_key {
        content = replace_toml_value(&content, "anthropic_api_key", &format!("\"{}\"", key));
    }
    if let Some(ref wid) = payload.workspace_id {
        content = replace_toml_value(&content, "notion_workspace_id", &format!("\"{}\"", wid));
    }
    if let Some(ref repos) = payload.github_repos {
        let repos_str: Vec<String> = repos.iter().map(|r| format!("\"{}\"", r)).collect();
        content = replace_toml_value(&content, "repos", &format!("[{}]", repos_str.join(", ")));
    }
    if let Some(ref secret) = payload.webhook_secret {
        content = replace_toml_value(&content, "webhook_secret", &format!("\"{}\"", secret));
    }
    if let Some(ref host) = payload.server_host {
        content = replace_toml_value(&content, "host", &format!("\"{}\"", host));
    }
    if let Some(port) = payload.server_port {
        content = replace_toml_value(&content, "port", &port.to_string());
    }

    std::fs::write(path, content)?;
    Ok(())
}

/// Replace a TOML key's value on its line, preserving the rest.
fn replace_toml_value(content: &str, key: &str, new_value: &str) -> String {
    let mut result = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(key) && trimmed.contains('=') {
            // Find the key = ... pattern
            if let Some(eq_pos) = line.find('=') {
                // Preserve everything before and including '= '
                let prefix = &line[..eq_pos + 1];
                result.push_str(prefix);
                result.push(' ');
                result.push_str(new_value);
            } else {
                result.push_str(line);
            }
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    result
}

// ═══════════════════════════════════════════════════════════════
//  GitHub API Proxy
// ═══════════════════════════════════════════════════════════════

/// GET /api/github/repos — fetch details for all configured repos
async fn api_github_repos(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let c = cfg(&state);
    let token = c.auth.github_token.clone();
    let client = match github_client(&token) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let mut results = Vec::new();
    for repo_slug in &c.github.repos {
        let url = format!("https://api.github.com/repos/{}", repo_slug);
        match client.get(&url).send().await {
            Ok(resp) => {
                match resp.json::<serde_json::Value>().await {
                    Ok(json) => results.push(json),
                    Err(e) => {
                        error!("Failed to parse GitHub response for {}: {e}", repo_slug);
                        results.push(serde_json::json!({"error": e.to_string(), "repo": repo_slug}));
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch GitHub repo {}: {e}", repo_slug);
                results.push(serde_json::json!({"error": e.to_string(), "repo": repo_slug}));
            }
        }
    }

    Json(serde_json::json!(results)).into_response()
}

/// GET /api/github/repo/:owner/:repo — proxy to GitHub repo detail
async fn api_github_repo_detail(
    State(state): State<Arc<AppState>>,
    Path((owner, repo)): Path<(String, String)>,
) -> impl IntoResponse {
    let token = cfg(&state).auth.github_token.clone();
    let client = match github_client(&token) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => Json(json).into_response(),
            Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        },
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/github/repo/:owner/:repo/pulls — proxy to GitHub pulls
async fn api_github_pulls(
    State(state): State<Arc<AppState>>,
    Path((owner, repo)): Path<(String, String)>,
) -> impl IntoResponse {
    let token = cfg(&state).auth.github_token.clone();
    let client = match github_client(&token) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let url = format!(
        "https://api.github.com/repos/{}/{}/pulls?state=all&per_page=30&sort=updated&direction=desc",
        owner, repo
    );
    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => Json(json).into_response(),
            Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        },
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/github/repo/:owner/:repo/issues — proxy to GitHub issues
async fn api_github_issues(
    State(state): State<Arc<AppState>>,
    Path((owner, repo)): Path<(String, String)>,
) -> impl IntoResponse {
    let token = cfg(&state).auth.github_token.clone();
    let client = match github_client(&token) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let url = format!(
        "https://api.github.com/repos/{}/{}/issues?state=all&per_page=30&sort=updated&direction=desc",
        owner, repo
    );
    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => Json(json).into_response(),
            Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        },
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/github/repo/:owner/:repo/commits — proxy to GitHub commits
async fn api_github_commits(
    State(state): State<Arc<AppState>>,
    Path((owner, repo)): Path<(String, String)>,
) -> impl IntoResponse {
    let token = cfg(&state).auth.github_token.clone();
    let client = match github_client(&token) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let url = format!(
        "https://api.github.com/repos/{}/{}/commits?per_page=30",
        owner, repo
    );
    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => Json(json).into_response(),
            Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        },
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

/// GET /api/github/repo/:owner/:repo/contributors — proxy to GitHub contributors
async fn api_github_contributors(
    State(state): State<Arc<AppState>>,
    Path((owner, repo)): Path<(String, String)>,
) -> impl IntoResponse {
    let token = cfg(&state).auth.github_token.clone();
    let client = match github_client(&token) {
        Ok(c) => c,
        Err(e) => {
            return (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response();
        }
    };

    let url = format!(
        "https://api.github.com/repos/{}/{}/contributors?per_page=30",
        owner, repo
    );
    match client.get(&url).send().await {
        Ok(resp) => match resp.json::<serde_json::Value>().await {
            Ok(json) => Json(json).into_response(),
            Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
        },
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({"error": e.to_string()}))).into_response(),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Setup API — Notion database initialization from dashboard
// ═══════════════════════════════════════════════════════════════

/// GET /api/setup/status — check if Notion databases are initialized
async fn api_setup_status(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let c = cfg(&state);
    let db = &c.databases;

    let configured = !db.projects.is_empty();
    let db_count = [
        &db.projects, &db.rfcs, &db.rfc_comments, &db.benchmarks,
        &db.regressions, &db.performance_baselines, &db.dependencies,
        &db.audit_runs, &db.modules, &db.onboarding_tracks,
        &db.onboarding_steps, &db.knowledge_gaps, &db.env_config,
        &db.config_snapshots, &db.secret_rotation_log, &db.pr_reviews,
        &db.review_playbook, &db.review_patterns, &db.tech_debt,
        &db.health_reports, &db.engineering_digest, &db.events, &db.releases,
    ]
    .iter()
    .filter(|id| !id.is_empty())
    .count();

    let notion_ready = !c.auth.notion_mcp_token.is_empty();
    let github_ready = !c.auth.github_token.is_empty();
    let ai_ready = !c.auth.anthropic_api_key.is_empty();

    Json(serde_json::json!({
        "initialized": configured,
        "databases_configured": db_count,
        "databases_total": 23,
        "notion_ready": notion_ready,
        "github_ready": github_ready,
        "ai_ready": ai_ready,
        "workspace_id": c.workspace.notion_workspace_id,
    }))
}

/// POST /api/setup/notion — create all Notion databases, relations, and seed data
async fn api_setup_notion(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    info!("[API] Notion setup requested from dashboard");

    let c = cfg(&state);
    if c.auth.notion_mcp_token.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "Notion integration token must be configured first. Go to Settings."
        }))).into_response();
    }

    let n = notion(&state);
    let workspace_id = c.workspace.notion_workspace_id.clone();

    // Step 1: Create all databases (auto-creates parent page at workspace root)
    let db_ids = match crate::setup::create_all_databases(&n, &workspace_id).await {
        Ok(ids) => ids,
        Err(e) => {
            error!("[Setup] Failed to create databases: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "error": format!("Database creation failed: {e}")
            }))).into_response();
        }
    };

    // Step 2: Create relations
    if let Err(e) = crate::setup::create_relations(&n, &db_ids).await {
        warn!("[Setup] Some relations failed (non-fatal): {e}");
    }

    // Step 3: Create default playbook
    if let Err(e) = crate::setup::create_default_playbook(&n, &db_ids.review_playbook).await {
        warn!("[Setup] Playbook rules failed (non-fatal): {e}");
    }

    // Step 4: Create sample project
    if let Err(e) = crate::setup::create_sample_project(&n, &db_ids.projects).await {
        warn!("[Setup] Sample project failed (non-fatal): {e}");
    }

    // Step 5: Persist to engram.toml
    let config_path = state.config_path.clone();
    if let Err(e) = crate::setup::persist_database_ids(&config_path, &db_ids) {
        error!("[Setup] Failed to write database IDs to config: {e}");
    }

    // Step 6: Update in-memory config with new database IDs
    {
        let mut config = state.config.write().unwrap();
        config.databases = db_ids.clone();
    }

    info!("[Setup] Notion setup complete — 23 databases, 18 relations, 3 rules");

    // Step 7: Dispatch SetupComplete event to trigger all intelligence layers
    let project_id = db_ids.projects.clone();
    state.router.dispatch(engram_types::events::EngramEvent::SetupComplete {
        project_id,
    });
    info!("[Setup] SetupComplete event dispatched — intelligence layers will auto-generate data");

    // Get the ENGRAM page ID to return to the dashboard
    let engram_page_id = crate::setup::get_engram_page_id(&n).await
        .unwrap_or_default();

    Json(serde_json::json!({
        "status": "ok",
        "message": "Notion workspace initialized. Intelligence layers are generating data automatically.",
        "databases_created": 23,
        "relations_created": 18,
        "playbook_rules": 3,
        "engram_page_id": engram_page_id,
    })).into_response()
}

// ═══════════════════════════════════════════════════════════════
//  POST /api/intelligence/generate — trigger all intelligence layers
// ═══════════════════════════════════════════════════════════════

/// Manually trigger all intelligence agents to generate/refresh their data.
/// This dispatches SetupComplete + all scheduled triggers so every agent runs.
async fn api_intelligence_generate(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let c = cfg(&state);
    let project_id = c.databases.projects.clone();

    if project_id.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "error": "No databases configured. Run Notion setup first."
        }))).into_response();
    }

    info!("[API] Intelligence generation triggered manually");

    // Dispatch all trigger events to wake up every agent
    use engram_types::events::EngramEvent;
    state.router.dispatch(EngramEvent::SetupComplete { project_id: project_id.clone() });
    state.router.dispatch(EngramEvent::DailyAuditTrigger { project_id: project_id.clone() });
    state.router.dispatch(EngramEvent::WeeklyDigestTrigger { project_id: project_id.clone() });
    state.router.dispatch(EngramEvent::WeeklyRfcStalenessTrigger { project_id: project_id.clone() });
    state.router.dispatch(EngramEvent::DailyRotationCheckTrigger { project_id: project_id.clone() });
    state.router.dispatch(EngramEvent::WeeklyKnowledgeGapTrigger { project_id: project_id.clone() });

    // Trigger onboarding document generation for each configured repo
    let repos = c.github.repos.clone();
    for repo in &repos {
        state.router.dispatch(EngramEvent::NewEngineerOnboards {
            engineer_name: "New Maintainer".to_string(),
            role: engram_types::events::Role::FullStack,
            project_id: project_id.clone(),
            repo: repo.clone(),
        });
    }

    Json(serde_json::json!({
        "status": "ok",
        "message": "All intelligence layers triggered. Data will populate in Notion within minutes.",
    })).into_response()
}
