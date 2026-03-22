//! Timeline Agent — unified event stream aggregator.
//!
//! Handles ALL EngramEvent variants by logging them to the ENGRAM/Events
//! database in Notion. Each event is mapped to an appropriate
//! TimelineEventType and Severity, then written as a new Event record.

use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{info, warn, error};
use uuid::Uuid;

use engram_types::clients::{AgentContext, properties as prop};
use engram_types::events::{EngramEvent, Severity as EventSeverity};
use engram_types::events::{SourceLayer, TimelineEventType};
use engram_types::notion_schema::events as schema;

/// Downcast the shared state to AgentContext.
fn get_ctx(state: &Arc<dyn std::any::Any + Send + Sync>) -> Option<&AgentContext> {
    state.downcast_ref::<AgentContext>()
}

/// Run the Timeline-agent main loop
pub async fn run(state: Arc<dyn std::any::Any + Send + Sync>, mut rx: broadcast::Receiver<EngramEvent>) {
    info!("[TimelineAgent] Started");
    loop {
        match rx.recv().await {
            Ok(event) => {
                if let Err(e) = handle_event(&state, &event).await {
                    error!("[TimelineAgent] Error handling event: {e}");
                }
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                warn!("[TimelineAgent] Lagged behind by {n} events");
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("[TimelineAgent] Channel closed, shutting down");
                break;
            }
        }
    }
}

async fn handle_event(state: &Arc<dyn std::any::Any + Send + Sync>, event: &EngramEvent) -> anyhow::Result<()> {
    if matches!(event, EngramEvent::SetupComplete { .. }) {
        if let EngramEvent::SetupComplete { project_id } = event {
            info!("[TimelineAgent] SetupComplete — timeline is ready for project {project_id}");
        }
        return Ok(());
    }
    let mapped = map_event(event);
    if let Some(timeline_event) = mapped {
        write_timeline_event(state, &timeline_event).await?;
    }
    Ok(())
}

/// A fully resolved timeline event ready to write to Notion.
struct TimelineRecord {
    event_id: String,
    event_type: TimelineEventType,
    title: String,
    severity: EventSeverity,
    source_layer: SourceLayer,
    project_id: Option<String>,
    related_rfc: Option<String>,
    related_regression: Option<String>,
    related_dependency: Option<String>,
    related_pr: Option<String>,
    related_env_var: Option<String>,
    actor: Option<String>,
    requires_action: bool,
    occurred_at: String,
}

impl TimelineRecord {
    fn new(
        event_type: TimelineEventType,
        title: String,
        severity: EventSeverity,
        source_layer: SourceLayer,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            event_type,
            title,
            severity,
            source_layer,
            project_id: None,
            related_rfc: None,
            related_regression: None,
            related_dependency: None,
            related_pr: None,
            related_env_var: None,
            actor: None,
            requires_action: false,
            occurred_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn with_project(mut self, project_id: &str) -> Self {
        self.project_id = Some(project_id.to_string());
        self
    }

    fn with_rfc(mut self, rfc_id: &str) -> Self {
        self.related_rfc = Some(rfc_id.to_string());
        self
    }

    fn with_regression(mut self, regression_id: &str) -> Self {
        self.related_regression = Some(regression_id.to_string());
        self
    }

    fn with_dependency(mut self, dep_id: &str) -> Self {
        self.related_dependency = Some(dep_id.to_string());
        self
    }

    fn with_pr(mut self, pr_ref: &str) -> Self {
        self.related_pr = Some(pr_ref.to_string());
        self
    }

    fn with_env_var(mut self, var_id: &str) -> Self {
        self.related_env_var = Some(var_id.to_string());
        self
    }

    fn with_actor(mut self, actor: &str) -> Self {
        self.actor = Some(actor.to_string());
        self
    }

    fn with_action_required(mut self) -> Self {
        self.requires_action = true;
        self
    }
}

/// Map each EngramEvent variant to a TimelineRecord with appropriate type and severity.
fn map_event(event: &EngramEvent) -> Option<TimelineRecord> {
    let record = match event {
        EngramEvent::PrOpened { repo, pr_number, title, author, .. } => {
            TimelineRecord::new(
                TimelineEventType::PrMerged, // Reusing closest type; ideally we'd add PrOpened
                format!("PR #{pr_number} opened: {title}"),
                EventSeverity::Info,
                SourceLayer::Review,
            )
            .with_pr(&format!("{repo}#{pr_number}"))
            .with_actor(author)
        }

        EngramEvent::PrMerged { repo, pr_number, title, author, rfc_references, .. } => {
            let mut rec = TimelineRecord::new(
                TimelineEventType::PrMerged,
                format!("PR #{pr_number} merged: {title}"),
                EventSeverity::Info,
                SourceLayer::Review,
            )
            .with_pr(&format!("{repo}#{pr_number}"))
            .with_actor(author);

            if let Some(rfc_ref) = rfc_references.first() {
                rec = rec.with_rfc(rfc_ref);
            }
            rec
        }

        EngramEvent::CiBenchmarkPosted { project_id, commit_sha, .. } => {
            TimelineRecord::new(
                TimelineEventType::ModuleUpdated,
                format!("CI benchmark posted for commit {}", &commit_sha[..8.min(commit_sha.len())]),
                EventSeverity::Info,
                SourceLayer::Pulse,
            )
            .with_project(project_id)
        }

        EngramEvent::CiAuditPosted { project_id, tool, commit_sha, .. } => {
            TimelineRecord::new(
                TimelineEventType::CveFound,
                format!("Audit run ({tool}) posted for commit {}", &commit_sha[..8.min(commit_sha.len())]),
                EventSeverity::Info,
                SourceLayer::Shield,
            )
            .with_project(project_id)
        }

        EngramEvent::RfcCreated { rfc_id, project_id, rfc_notion_page_id, .. } => {
            TimelineRecord::new(
                TimelineEventType::RfcCreated,
                format!("RFC {rfc_id} created"),
                EventSeverity::Info,
                SourceLayer::Decisions,
            )
            .with_project(project_id)
            .with_rfc(rfc_notion_page_id)
        }

        EngramEvent::RfcApproved { rfc_id, project_id, rfc_notion_page_id, .. } => {
            TimelineRecord::new(
                TimelineEventType::RfcApproved,
                format!("RFC {rfc_id} approved"),
                EventSeverity::Info,
                SourceLayer::Decisions,
            )
            .with_project(project_id)
            .with_rfc(rfc_notion_page_id)
        }

        EngramEvent::RegressionDetected {
            regression_notion_page_id, severity, metric_name, delta_pct, project_id, related_pr,
        } => {
            let mut rec = TimelineRecord::new(
                TimelineEventType::RegressionDetected,
                format!("Regression detected: {metric_name} ({delta_pct:+.1}%)"),
                *severity,
                SourceLayer::Pulse,
            )
            .with_project(project_id)
            .with_regression(regression_notion_page_id)
            .with_action_required();

            if let Some(pr) = related_pr {
                rec = rec.with_pr(pr);
            }
            rec
        }

        EngramEvent::CveDetected {
            dependency_notion_page_id, package_name, cve_ids, severity, project_id,
        } => {
            let cve_list = cve_ids.join(", ");
            TimelineRecord::new(
                TimelineEventType::CveFound,
                format!("CVE detected in {package_name}: {cve_list}"),
                *severity,
                SourceLayer::Shield,
            )
            .with_project(project_id)
            .with_dependency(dependency_notion_page_id)
            .with_action_required()
        }

        EngramEvent::SecretRotationDue { var_notion_page_id, var_name, days_overdue, project_id } => {
            let sev = if *days_overdue > 30 {
                EventSeverity::High
            } else if *days_overdue > 7 {
                EventSeverity::Medium
            } else {
                EventSeverity::Low
            };
            TimelineRecord::new(
                TimelineEventType::RotationDue,
                format!("Secret rotation overdue: {var_name} ({days_overdue} days)"),
                sev,
                SourceLayer::Vault,
            )
            .with_project(project_id)
            .with_env_var(var_notion_page_id)
            .with_action_required()
        }

        EngramEvent::EnvVarMissingInProd { var_notion_page_id, var_name, project_id } => {
            TimelineRecord::new(
                TimelineEventType::ConfigMismatch,
                format!("Env var missing in production: {var_name}"),
                EventSeverity::High,
                SourceLayer::Vault,
            )
            .with_project(project_id)
            .with_env_var(var_notion_page_id)
            .with_action_required()
        }

        EngramEvent::NewEngineerOnboards { engineer_name, role, project_id, repo } => {
            TimelineRecord::new(
                TimelineEventType::NewEngineer,
                format!("New engineer onboarded: {engineer_name} ({role}) for {repo}"),
                EventSeverity::Info,
                SourceLayer::Atlas,
            )
            .with_project(project_id)
            .with_actor(engineer_name)
        }

        EngramEvent::ReviewPatternCreated { pattern_notion_page_id, pattern_name, frequency, project_id } => {
            let sev = if *frequency > 10 {
                EventSeverity::Medium
            } else {
                EventSeverity::Low
            };
            TimelineRecord::new(
                TimelineEventType::DebtCreated,
                format!("Review pattern identified: {pattern_name} (freq: {frequency})"),
                sev,
                SourceLayer::Review,
            )
            .with_project(project_id)
            .with_pr(pattern_notion_page_id) // closest relation
        }

        EngramEvent::WeeklyDigestTrigger { project_id } => {
            TimelineRecord::new(
                TimelineEventType::HealthReport,
                "Weekly health digest triggered".to_string(),
                EventSeverity::Info,
                SourceLayer::Decisions, // Cross-cutting, attribute to governance
            )
            .with_project(project_id)
        }

        EngramEvent::DailyAuditTrigger { project_id } => {
            TimelineRecord::new(
                TimelineEventType::CveFound,
                "Daily audit scan triggered".to_string(),
                EventSeverity::Info,
                SourceLayer::Shield,
            )
            .with_project(project_id)
        }

        EngramEvent::WeeklyRfcStalenessTrigger { project_id } => {
            TimelineRecord::new(
                TimelineEventType::RfcCreated,
                "Weekly RFC staleness check triggered".to_string(),
                EventSeverity::Info,
                SourceLayer::Decisions,
            )
            .with_project(project_id)
        }

        EngramEvent::DailyRotationCheckTrigger { project_id } => {
            TimelineRecord::new(
                TimelineEventType::RotationDue,
                "Daily secret rotation check triggered".to_string(),
                EventSeverity::Info,
                SourceLayer::Vault,
            )
            .with_project(project_id)
        }

        EngramEvent::WeeklyKnowledgeGapTrigger { project_id } => {
            TimelineRecord::new(
                TimelineEventType::ModuleUpdated,
                "Weekly knowledge gap scan triggered".to_string(),
                EventSeverity::Info,
                SourceLayer::Atlas,
            )
            .with_project(project_id)
        }

        EngramEvent::ReleaseCreated { project_id, version, milestone } => {
            TimelineRecord::new(
                TimelineEventType::HealthReport, // Closest for release events
                format!("Release {version} created (milestone: {milestone})"),
                EventSeverity::Info,
                SourceLayer::Review, // Cross-cutting, attribute to review as release quality
            )
            .with_project(project_id)
        }

        EngramEvent::SetupComplete { .. } => {
            // Handled in handle_event before map_event is called
            return None;
        }
    };

    Some(record)
}

/// Write a TimelineRecord to the ENGRAM/Events database via Notion API.
async fn write_timeline_event(
    state: &Arc<dyn std::any::Any + Send + Sync>,
    record: &TimelineRecord,
) -> anyhow::Result<()> {
    let ctx = match get_ctx(state) {
        Some(c) => c,
        None => {
            error!("[TimelineAgent] Failed to downcast state to AgentContext");
            anyhow::bail!("AgentContext downcast failed");
        }
    };

    let db_id = &ctx.config.databases.events;

    // Build a details summary from all optional context
    let mut detail_parts: Vec<String> = Vec::new();
    detail_parts.push(format!("severity={}", record.severity));
    if record.requires_action { detail_parts.push("requires_action=true".into()); }
    if let Some(ref rfc) = record.related_rfc { detail_parts.push(format!("rfc={rfc}")); }
    if let Some(ref reg) = record.related_regression { detail_parts.push(format!("regression={reg}")); }
    if let Some(ref dep) = record.related_dependency { detail_parts.push(format!("dependency={dep}")); }
    if let Some(ref pr) = record.related_pr { detail_parts.push(format!("pr={pr}")); }
    if let Some(ref var) = record.related_env_var { detail_parts.push(format!("env_var={var}")); }
    if let Some(ref actor) = record.actor { detail_parts.push(format!("actor={actor}")); }
    let details = detail_parts.join(" | ");

    // Build properties matching actual Events DB schema
    let mut props = serde_json::json!({
        schema::TITLE:         prop::title(&record.title),
        schema::SOURCE_LAYER:  prop::select(&record.source_layer.to_string()),
        schema::TYPE:          prop::rich_text(&record.event_type.to_string()),
        schema::DETAILS:       prop::rich_text(&details),
        schema::TIMESTAMP:     prop::date(&record.occurred_at),
        schema::IS_MILESTONE:  prop::checkbox(record.requires_action),
    });

    if let Some(ref project) = record.project_id {
        props[schema::PROJECT] = prop::rich_text(project);
    }

    match ctx.notion.create_page(db_id, props).await {
        Ok(_response) => {
            info!(
                "[TimelineAgent] Created event: {} | {} | {} | layer={}",
                record.event_id, record.event_type, record.title, record.source_layer,
            );
        }
        Err(e) => {
            error!(
                "[TimelineAgent] Failed to create event {}: {e}",
                record.event_id,
            );
            return Err(e);
        }
    }

    Ok(())
}
