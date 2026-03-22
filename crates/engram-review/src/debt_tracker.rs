//! Tech Debt promotion logic.
//!
//! When a review pattern's cumulative frequency exceeds a configurable
//! threshold it gets promoted to a Tech Debt item in the ENGRAM/Tech Debt DB.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::prompts;

/// Default threshold — a pattern must appear at least this many times across
/// PRs before being promoted to a Tech Debt item.
pub const DEFAULT_PROMOTION_THRESHOLD: u32 = 5;

// ─── Data structures ────────────────────────────────────────────────

/// A Tech Debt item ready to be written to Notion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebtItem {
    pub title: String,
    pub description: String,
    pub severity: String,
    pub effort_estimate: String,
    pub suggested_approach: String,
    pub source_pattern: String,
    pub pattern_frequency: u32,
}

// ─── Promotion logic ────────────────────────────────────────────────

/// Check whether a pattern's frequency exceeds the promotion threshold.
///
/// If it does, build a `DebtItem` (in production, Claude generates the
/// description). Returns `None` when the pattern has not yet reached the
/// threshold.
pub async fn check_debt_promotion(
    pattern_name: &str,
    frequency: u32,
    threshold: u32,
    category: &str,
) -> anyhow::Result<Option<DebtItem>> {
    if frequency < threshold {
        info!(
            "[ReviewAgent] Pattern '{}' frequency {} < threshold {} — no promotion",
            pattern_name, frequency, threshold
        );
        return Ok(None);
    }

    info!(
        "[ReviewAgent] Pattern '{}' frequency {} >= threshold {} — promoting to Tech Debt",
        pattern_name, frequency, threshold
    );

    // Build the Claude prompt (would be sent to Claude in production)
    let prompt = prompts::tech_debt_promotion_prompt(pattern_name, frequency, threshold, category);
    info!(
        "[ReviewAgent] Would call Claude for: Tech Debt promotion of '{}' — prompt length {} chars",
        pattern_name,
        prompt.len()
    );

    // Placeholder debt item until Claude integration is live.
    let debt = DebtItem {
        title: format!("Address recurring pattern: {pattern_name}"),
        description: format!(
            "The review pattern '{pattern_name}' (category: {category}) has been observed \
             {frequency} times across PRs, exceeding the promotion threshold of {threshold}. \
             This should be addressed systematically."
        ),
        severity: if frequency >= threshold * 3 {
            "High".into()
        } else if frequency >= threshold * 2 {
            "Medium".into()
        } else {
            "Low".into()
        },
        effort_estimate: "TBD".into(),
        suggested_approach: format!(
            "Review recent PRs exhibiting '{pattern_name}' and create an RFC \
             to address the root cause."
        ),
        source_pattern: pattern_name.to_string(),
        pattern_frequency: frequency,
    };

    // Log the Notion write that would happen.
    info!(
        "[ReviewAgent] Would call MCP: notion_create_page with \
         {{\"database\": \"ENGRAM/Tech Debt\", \"debt_item\": \"{}\", \
         \"source\": \"Review Pattern\", \"source_pattern\": \"{}\", \
         \"severity\": \"{}\"}}",
        debt.title, pattern_name, debt.severity
    );

    Ok(Some(debt))
}

/// Parse Claude's JSON response for a tech debt promotion into a `DebtItem`.
pub fn parse_debt_response(json_str: &str, pattern_name: &str, frequency: u32) -> anyhow::Result<DebtItem> {
    #[derive(Deserialize)]
    struct RawDebt {
        debt_title: String,
        description: String,
        severity: String,
        effort_estimate: String,
        suggested_approach: String,
    }

    let raw: RawDebt = serde_json::from_str(json_str)?;
    Ok(DebtItem {
        title: raw.debt_title,
        description: raw.description,
        severity: raw.severity,
        effort_estimate: raw.effort_estimate,
        suggested_approach: raw.suggested_approach,
        source_pattern: pattern_name.to_string(),
        pattern_frequency: frequency,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_below_threshold_no_promotion() {
        let result = check_debt_promotion("unwrap-usage", 3, 5, "error-handling")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_at_threshold_promotes() {
        let result = check_debt_promotion("unwrap-usage", 5, 5, "error-handling")
            .await
            .unwrap();
        assert!(result.is_some());
        let debt = result.unwrap();
        assert!(debt.title.contains("unwrap-usage"));
        assert_eq!(debt.severity, "Low");
    }

    #[tokio::test]
    async fn test_high_frequency_severity() {
        let result = check_debt_promotion("missing-tests", 20, 5, "testing")
            .await
            .unwrap();
        let debt = result.unwrap();
        assert_eq!(debt.severity, "High");
    }

    #[test]
    fn test_parse_debt_response() {
        let json = r#"{
            "debt_title": "Fix unwrap usage",
            "description": "Replace unwrap with proper error handling",
            "severity": "Medium",
            "effort_estimate": "2d",
            "suggested_approach": "Use anyhow Result everywhere"
        }"#;
        let debt = parse_debt_response(json, "unwrap-usage", 7).unwrap();
        assert_eq!(debt.title, "Fix unwrap usage");
        assert_eq!(debt.source_pattern, "unwrap-usage");
        assert_eq!(debt.pattern_frequency, 7);
    }
}
