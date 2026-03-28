use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level ENGRAM configuration, deserialized from engram.toml
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngramConfig {
    pub workspace: WorkspaceConfig,
    pub auth: AuthConfig,
    pub server: ServerConfig,
    pub thresholds: ThresholdConfig,
    pub schedule: ScheduleConfig,
    pub claude: ClaudeConfig,
    pub databases: DatabaseIds,
    #[serde(default)]
    pub github: GitHubConfig,
    #[serde(default)]
    pub user: UserProfile,
}

/// Dashboard user profile — created during first-start setup
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserProfile {
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password_hash: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub avatar_initials: String,
    /// Random 64-char hex string generated on first setup, used to sign JWTs
    #[serde(default)]
    pub jwt_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceConfig {
    pub notion_workspace_id: String,
    #[serde(default)]
    pub parent_page_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub notion_mcp_token: String,
    #[serde(default)]
    pub anthropic_api_key: String,
    #[serde(default)]
    pub github_token: String,
    #[serde(default)]
    pub webhook_secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThresholdConfig {
    pub warning_delta_pct: f64,
    pub critical_delta_pct: f64,
    pub production_impact_delta_pct: f64,
    pub baseline_window: usize,
    pub pattern_debt_threshold: u32,
    pub auto_rfc_severities: Vec<String>,
    pub rfc_stale_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub daily_audit: String,
    pub weekly_digest: String,
    pub weekly_rfc_staleness: String,
    pub daily_rotation_check: String,
    pub weekly_knowledge_gap_scan: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeConfig {
    pub model: String,
    pub max_tokens: u32,
}

/// GitHub configuration — repos tracked via webhooks
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitHubConfig {
    #[serde(default)]
    pub repos: Vec<String>,
}

/// Notion database IDs — populated by the dashboard setup wizard
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DatabaseIds {
    pub projects: String,
    pub rfcs: String,
    pub rfc_comments: String,
    pub benchmarks: String,
    pub regressions: String,
    pub performance_baselines: String,
    pub dependencies: String,
    pub audit_runs: String,
    pub modules: String,
    pub onboarding_tracks: String,
    pub onboarding_steps: String,
    pub knowledge_gaps: String,
    pub env_config: String,
    pub config_snapshots: String,
    pub secret_rotation_log: String,
    pub pr_reviews: String,
    pub review_playbook: String,
    pub review_patterns: String,
    pub tech_debt: String,
    pub health_reports: String,
    pub engineering_digest: String,
    pub events: String,
    pub releases: String,
}

impl EngramConfig {
    /// Load configuration from engram.toml, with env var overrides
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let mut config: EngramConfig = toml::from_str(&content)?;

        // Environment variable overrides (only when non-empty)
        if let Ok(val) = std::env::var("NOTION_MCP_TOKEN") {
            if !val.is_empty() { config.auth.notion_mcp_token = val; }
        }
        if let Ok(val) = std::env::var("ANTHROPIC_API_KEY") {
            if !val.is_empty() { config.auth.anthropic_api_key = val; }
        }
        if let Ok(val) = std::env::var("GITHUB_TOKEN") {
            if !val.is_empty() { config.auth.github_token = val; }
        }
        if let Ok(val) = std::env::var("WEBHOOK_SECRET") {
            if !val.is_empty() { config.auth.webhook_secret = val; }
        }
        if let Ok(val) = std::env::var("NOTION_WORKSPACE_ID") {
            if !val.is_empty() { config.workspace.notion_workspace_id = val; }
        }
        if let Ok(val) = std::env::var("ENGRAM_HOST") {
            if !val.is_empty() { config.server.host = val; }
        }
        if let Ok(val) = std::env::var("ENGRAM_PORT") {
            if let Ok(port) = val.parse() {
                config.server.port = port;
            }
        }
        if let Ok(val) = std::env::var("CLAUDE_MODEL") {
            if !val.is_empty() { config.claude.model = val; }
        }
        if let Ok(val) = std::env::var("ENGRAM_JWT_SECRET") {
            if !val.is_empty() { config.user.jwt_secret = val; }
        }

        Ok(config)
    }
}
