//! Onboarding track scaffolding: generates role-specific onboarding tracks.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::prompts::{self, ModuleSummaryContext};

/// A complete onboarding track for a new engineer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingTrack {
    /// Display name for the track.
    pub name: String,
    /// The role this track is designed for (e.g. "Backend", "DevOps").
    pub role: String,
    /// Total estimated hours to complete the track.
    pub estimated_hours: u32,
    /// Ordered sequence of onboarding steps.
    pub steps: Vec<OnboardingStep>,
}

/// A single step in an onboarding track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingStep {
    /// Short title for the step.
    pub title: String,
    /// When this step should be done, e.g. "Week 1 / Day 1".
    pub week_day: String,
    /// Classification of the step.
    pub step_type: StepType,
    /// 2-4 sentence description of what the engineer should do.
    pub description: String,
    /// Estimated time, e.g. "2h", "30m".
    pub estimated_time: String,
    /// The module this step relates to, if any.
    pub related_module: Option<String>,
}

/// Classification of an onboarding step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StepType {
    Setup,
    Reading,
    HandsOn,
    Review,
}

impl std::fmt::Display for StepType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StepType::Setup => write!(f, "setup"),
            StepType::Reading => write!(f, "reading"),
            StepType::HandsOn => write!(f, "hands-on"),
            StepType::Review => write!(f, "review"),
        }
    }
}

/// Build the Claude prompt for generating an onboarding track.
///
/// Returns the prompt string to be sent to the LLM.
pub fn build_onboarding_prompt(
    role: &str,
    project_id: &str,
    module_summaries: &[ModuleSummaryContext],
    env_vars: &[String],
    recent_rfc_titles: &[String],
) -> String {
    info!(
        "[OnboardingGenerator] Building prompt for role={role} project={project_id} modules={} env_vars={} rfcs={}",
        module_summaries.len(),
        env_vars.len(),
        recent_rfc_titles.len()
    );
    prompts::onboarding_track_prompt(role, project_id, module_summaries, env_vars, recent_rfc_titles)
}

/// Parse a Claude JSON response into an [`OnboardingTrack`].
///
/// The response may contain markdown code fences; they are stripped before parsing.
pub fn parse_onboarding_track(claude_response: &str, role: &str) -> Result<OnboardingTrack> {
    let json_str = extract_json_block(claude_response);

    // Parse the raw JSON first to handle the flexible step_type and related_module fields.
    let raw: RawOnboardingTrack =
        serde_json::from_str(json_str).context("Failed to parse onboarding track JSON from Claude response")?;

    let steps: Vec<OnboardingStep> = raw
        .steps
        .into_iter()
        .map(|s| {
            let step_type = match s.step_type.to_lowercase().as_str() {
                "setup" => StepType::Setup,
                "reading" => StepType::Reading,
                "hands-on" | "hands_on" | "handson" => StepType::HandsOn,
                "review" => StepType::Review,
                _ => StepType::Reading, // default fallback
            };
            OnboardingStep {
                title: s.title,
                week_day: s.week_day,
                step_type,
                description: s.description,
                estimated_time: s.estimated_time,
                related_module: s.related_module,
            }
        })
        .collect();

    let track = OnboardingTrack {
        name: raw.track_name,
        role: role.to_string(),
        estimated_hours: raw.estimated_hours,
        steps,
    };

    info!(
        "[OnboardingGenerator] Parsed track '{}': {} steps, ~{}h",
        track.name,
        track.steps.len(),
        track.estimated_hours
    );

    Ok(track)
}

/// Generate a complete onboarding track (prompt + parse step combined).
///
/// In production the caller sends the prompt to Claude and passes the response here.
/// This function is the high-level API that composes the two halves.
pub fn generate_track(
    role: &str,
    project_id: &str,
    module_summaries: &[ModuleSummaryContext],
    env_vars: &[String],
    recent_rfc_titles: &[String],
    claude_response: &str,
) -> Result<OnboardingTrack> {
    info!("[OnboardingGenerator] Generating track for role={role}");
    // The prompt was already built and sent to Claude; we just parse the response.
    let _ = build_onboarding_prompt(role, project_id, module_summaries, env_vars, recent_rfc_titles);
    parse_onboarding_track(claude_response, role)
}

// ── Internal helpers ──

/// Raw JSON shape returned by Claude (before enum conversion).
#[derive(Deserialize)]
struct RawOnboardingTrack {
    track_name: String,
    estimated_hours: u32,
    steps: Vec<RawOnboardingStep>,
}

#[derive(Deserialize)]
struct RawOnboardingStep {
    title: String,
    week_day: String,
    step_type: String,
    description: String,
    estimated_time: String,
    related_module: Option<String>,
}

/// Extract the JSON body from a Claude response that may be wrapped in markdown fences.
fn extract_json_block(text: &str) -> &str {
    let trimmed = text.trim();

    if let Some(start) = trimmed.find("```json") {
        let after_fence = &trimmed[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim();
        }
    }

    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim();
        }
    }

    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_onboarding_track() {
        let response = r#"{
            "track_name": "Backend Onboarding",
            "estimated_hours": 40,
            "steps": [
                {
                    "title": "Environment Setup",
                    "week_day": "Week 1 / Day 1",
                    "step_type": "setup",
                    "description": "Install Rust toolchain. Configure env vars: DATABASE_URL, REDIS_URL.",
                    "estimated_time": "3h",
                    "related_module": null
                },
                {
                    "title": "Read Core Architecture RFC",
                    "week_day": "Week 1 / Day 2",
                    "step_type": "reading",
                    "description": "Read RFC-001 on system architecture. Take notes on data flow.",
                    "estimated_time": "2h",
                    "related_module": "engram-core"
                },
                {
                    "title": "Hands-on: Auth Module",
                    "week_day": "Week 2 / Day 1",
                    "step_type": "hands-on",
                    "description": "Write a small feature in the auth module.",
                    "estimated_time": "4h",
                    "related_module": "auth-module"
                }
            ]
        }"#;
        let track = parse_onboarding_track(response, "Backend").unwrap();
        assert_eq!(track.name, "Backend Onboarding");
        assert_eq!(track.role, "Backend");
        assert_eq!(track.estimated_hours, 40);
        assert_eq!(track.steps.len(), 3);
        assert_eq!(track.steps[0].step_type, StepType::Setup);
        assert_eq!(track.steps[1].step_type, StepType::Reading);
        assert_eq!(track.steps[2].step_type, StepType::HandsOn);
        assert!(track.steps[0].related_module.is_none());
        assert_eq!(track.steps[1].related_module.as_deref(), Some("engram-core"));
    }
}
