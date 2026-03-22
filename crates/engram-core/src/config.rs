use std::path::Path;

use anyhow::{Context, Result};
use engram_types::config::EngramConfig;
use tracing::info;

/// Load and validate ENGRAM configuration
pub fn load_config(path: &Path) -> Result<EngramConfig> {
    info!("Loading configuration from {}", path.display());

    let config = EngramConfig::load(path)
        .with_context(|| format!("Failed to load config from {}", path.display()))?;

    validate_config(&config)?;

    info!("Configuration loaded successfully");
    Ok(config)
}

fn validate_config(config: &EngramConfig) -> Result<()> {
    // Warn about missing tokens but don't fail — users configure via dashboard on first start
    if config.auth.notion_mcp_token.is_empty() {
        tracing::warn!("NOTION_MCP_TOKEN is empty — configure via the dashboard setup wizard");
    }
    if config.auth.anthropic_api_key.is_empty() {
        tracing::warn!("ANTHROPIC_API_KEY is empty — configure via the dashboard setup wizard");
    }
    if config.auth.github_token.is_empty() {
        tracing::warn!("GITHUB_TOKEN is empty — configure via the dashboard setup wizard");
    }
    if config.auth.webhook_secret.is_empty() {
        tracing::info!("WEBHOOK_SECRET is empty — webhooks will accept unsigned payloads until a secret is set");
    }
    Ok(())
}
