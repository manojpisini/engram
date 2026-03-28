use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use anyhow::Result;
use rust_embed::Embed;
use tracing::{info, error};

mod config;
mod webhook;
mod scheduler;
mod event_router;
mod notion_client;
mod claude_client;
pub mod setup;

use config::load_config;
use event_router::EventRouter;
use notion_client::NotionMcpClient;
use claude_client::ClaudeClient;
use engram_types::clients::AgentContext;

/// Shared application state available to all handlers and agents.
/// Config and Notion client are behind RwLock for runtime hot-reload
/// when users update tokens/repos from the dashboard.
pub struct AppState {
    pub config: RwLock<engram_types::config::EngramConfig>,
    pub config_path: PathBuf,
    pub notion: RwLock<NotionMcpClient>,
    pub claude: ClaudeClient,
    pub router: EventRouter,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env if present (optional — all config is done from the dashboard)
    dotenvy::dotenv().ok();

    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "engram=info,tower_http=info".into()),
        )
        .init();

    info!("ENGRAM — Engineering Intelligence, etched in Notion");
    info!("Starting ENGRAM core daemon...");

    // Determine config path
    let config_path = std::env::var("ENGRAM_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("engram.toml"));

    // Extract default config from embedded template if it doesn't exist
    if !config_path.exists() {
        info!("No config file found at {} — extracting default template", config_path.display());
        #[derive(Embed)]
        #[folder = "../../"]
        #[include = "engram.toml.example"]
        struct CfgTemplate;

        if let Some(tpl) = CfgTemplate::get("engram.toml.example") {
            if let Err(e) = std::fs::write(&config_path, tpl.data.as_ref()) {
                error!("Failed to write default config: {e}");
            } else {
                info!("Default config extracted to {}", config_path.display());
            }
        }
    }

    let config = load_config(&config_path)?;

    // Initialize shared clients
    let notion = NotionMcpClient::new(&config);
    let claude = ClaudeClient::new(&config);

    // Initialize event router with agent channels
    let router = EventRouter::new();

    let addr = format!("{}:{}", config.server.host, config.server.port);

    let state = Arc::new(AppState {
        config: RwLock::new(config.clone()),
        config_path: config_path.clone(),
        notion: RwLock::new(notion),
        claude,
        router,
    });

    // Create shared agent context for all layer agents
    let agent_ctx = Arc::new(AgentContext::new(&config));

    // Start layer agents
    start_agents(state.clone(), agent_ctx).await;

    // Start cron scheduler
    let scheduler_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = scheduler::start_scheduler(scheduler_state).await {
            error!("Scheduler failed: {e}");
        }
    });

    // Start webhook listener (axum HTTP server)
    info!("Webhook listener starting on {addr}");
    webhook::serve(state, &addr).await?;

    Ok(())
}

async fn start_agents(state: Arc<AppState>, ctx: Arc<AgentContext>) {
    info!("Starting layer agents...");

    macro_rules! spawn_agent {
        ($module:ident, $state:expr, $ctx:expr) => {{
            let rx = $state.router.subscribe();
            let c: Arc<dyn std::any::Any + Send + Sync> = $ctx.clone();
            tokio::spawn(async move {
                $module::agent::run(c, rx).await;
            });
        }};
    }

    spawn_agent!(engram_decisions, state, ctx);
    spawn_agent!(engram_pulse, state, ctx);
    spawn_agent!(engram_shield, state, ctx);
    spawn_agent!(engram_atlas, state, ctx);
    spawn_agent!(engram_vault, state, ctx);
    spawn_agent!(engram_review, state, ctx);
    spawn_agent!(engram_health, state, ctx);
    spawn_agent!(engram_timeline, state, ctx);
    spawn_agent!(engram_release, state, ctx);

    info!("All 9 layer agents started");
}
