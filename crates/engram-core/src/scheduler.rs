use std::sync::Arc;

use anyhow::Result;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::info;

use engram_types::events::EngramEvent;

use crate::AppState;

/// Start the cron scheduler for periodic ENGRAM tasks:
/// - Daily audit trigger
/// - Weekly digest
/// - Weekly RFC staleness check
/// - Daily rotation check
/// - Weekly knowledge gap scan
pub async fn start_scheduler(state: Arc<AppState>) -> Result<()> {
    let sched = JobScheduler::new().await?;

    // Read config once at scheduler startup for cron expressions
    let schedule_cfg = {
        let c = state.config.read().unwrap();
        c.schedule.clone()
    };

    // Daily audit trigger
    let s = state.clone();
    sched
        .add(Job::new_async(schedule_cfg.daily_audit.as_str(), move |_uuid, _lock| {
            let state = s.clone();
            Box::pin(async move {
                info!("[Scheduler] Daily audit trigger fired");
                let project_id = state.config.read().unwrap().databases.projects.clone();
                state.router.dispatch(EngramEvent::DailyAuditTrigger {
                    project_id,
                });
            })
        })?)
        .await?;

    // Weekly digest trigger
    let s = state.clone();
    sched
        .add(Job::new_async(schedule_cfg.weekly_digest.as_str(), move |_uuid, _lock| {
            let state = s.clone();
            Box::pin(async move {
                info!("[Scheduler] Weekly digest trigger fired");
                let project_id = state.config.read().unwrap().databases.projects.clone();
                state.router.dispatch(EngramEvent::WeeklyDigestTrigger {
                    project_id,
                });
            })
        })?)
        .await?;

    // Weekly RFC staleness check
    let s = state.clone();
    sched
        .add(Job::new_async(schedule_cfg.weekly_rfc_staleness.as_str(), move |_uuid, _lock| {
            let state = s.clone();
            Box::pin(async move {
                info!("[Scheduler] Weekly RFC staleness check fired");
                let project_id = state.config.read().unwrap().databases.projects.clone();
                state.router.dispatch(EngramEvent::WeeklyRfcStalenessTrigger {
                    project_id,
                });
            })
        })?)
        .await?;

    // Daily rotation check
    let s = state.clone();
    sched
        .add(Job::new_async(schedule_cfg.daily_rotation_check.as_str(), move |_uuid, _lock| {
            let state = s.clone();
            Box::pin(async move {
                info!("[Scheduler] Daily rotation check fired");
                let project_id = state.config.read().unwrap().databases.projects.clone();
                state.router.dispatch(EngramEvent::DailyRotationCheckTrigger {
                    project_id,
                });
            })
        })?)
        .await?;

    // Weekly knowledge gap scan
    let s = state.clone();
    sched
        .add(Job::new_async(schedule_cfg.weekly_knowledge_gap_scan.as_str(), move |_uuid, _lock| {
            let state = s.clone();
            Box::pin(async move {
                info!("[Scheduler] Weekly knowledge gap scan fired");
                let project_id = state.config.read().unwrap().databases.projects.clone();
                state.router.dispatch(EngramEvent::WeeklyKnowledgeGapTrigger {
                    project_id,
                });
            })
        })?)
        .await?;

    // GitHub data arrives via webhooks (configured per-repo by the user)
    // — no polling needed. See .github/workflows/engram-notify.yml for the GitHub Actions workflow.

    sched.start().await?;
    info!("Cron scheduler started with 5 scheduled jobs");

    // Keep the scheduler alive
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}
