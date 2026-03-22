//! Engineering Digest generation for the weekly health report.

use serde::{Deserialize, Serialize};

/// Aggregated weekly statistics for the Engineering Digest.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DigestData {
    /// Number of new RFCs created this week
    pub new_rfcs: u32,
    /// Number of RFCs approved this week
    pub rfcs_approved: u32,
    /// Number of performance regressions found this week
    pub regressions_found: u32,
    /// Number of regressions resolved this week
    pub regressions_resolved: u32,
    /// Number of new vulnerabilities discovered this week
    pub new_vulnerabilities: u32,
    /// Number of vulnerabilities triaged this week
    pub vulnerabilities_triaged: u32,
    /// Number of PRs reviewed this week
    pub prs_reviewed: u32,
    /// Number of new code patterns identified this week
    pub new_patterns: u32,
    /// Overall health score
    pub health_score: f64,
    /// Delta from last week's health score
    pub health_delta: f64,
    /// Notable events from the timeline
    pub notable_events: Vec<String>,
    /// Action items generated from analysis
    pub action_items: Vec<String>,
}

impl DigestData {
    /// Produce a human-readable summary string for embedding in prompts.
    pub fn to_summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("- New RFCs: {}", self.new_rfcs));
        lines.push(format!("- RFCs Approved: {}", self.rfcs_approved));
        lines.push(format!("- Regressions Found: {}", self.regressions_found));
        lines.push(format!("- Regressions Resolved: {}", self.regressions_resolved));
        lines.push(format!("- New Vulnerabilities: {}", self.new_vulnerabilities));
        lines.push(format!("- Vulnerabilities Triaged: {}", self.vulnerabilities_triaged));
        lines.push(format!("- PRs Reviewed: {}", self.prs_reviewed));
        lines.push(format!("- New Patterns: {}", self.new_patterns));

        if !self.notable_events.is_empty() {
            lines.push(String::new());
            lines.push("Notable Events:".to_string());
            for event in &self.notable_events {
                lines.push(format!("  - {event}"));
            }
        }

        if !self.action_items.is_empty() {
            lines.push(String::new());
            lines.push("Action Items:".to_string());
            for item in &self.action_items {
                lines.push(format!("  - {item}"));
            }
        }

        lines.join("\n")
    }
}

/// Generate the prompt for Claude to produce a digest narrative.
///
/// This prompt is used to get Claude to produce an overall weekly narrative
/// from raw digest statistics. The response is expected as plain text.
pub fn generate_digest_prompt(data: &DigestData) -> String {
    format!(
        r#"You are ENGRAM's Engineering Digest generator. Summarize the following weekly statistics into a concise 2-3 paragraph narrative suitable for a Slack post or email digest.

## Weekly Statistics
{summary}

## Health Score
Current: {score:.1} (delta: {delta:+.1} from last week)

Write a brief, actionable summary highlighting the most important changes this week. Focus on what the team should pay attention to. Use plain text, no JSON."#,
        summary = data.to_summary(),
        score = data.health_score,
        delta = data.health_delta,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_digest_data_default() {
        let data = DigestData::default();
        assert_eq!(data.new_rfcs, 0);
        assert_eq!(data.health_score, 0.0);
    }

    #[test]
    fn test_to_summary_basic() {
        let data = DigestData {
            new_rfcs: 3,
            rfcs_approved: 1,
            regressions_found: 2,
            prs_reviewed: 15,
            ..Default::default()
        };
        let summary = data.to_summary();
        assert!(summary.contains("New RFCs: 3"));
        assert!(summary.contains("PRs Reviewed: 15"));
    }

    #[test]
    fn test_to_summary_with_events() {
        let data = DigestData {
            notable_events: vec!["Critical CVE found in openssl".to_string()],
            action_items: vec!["Upgrade openssl to 3.1.5".to_string()],
            ..Default::default()
        };
        let summary = data.to_summary();
        assert!(summary.contains("Notable Events:"));
        assert!(summary.contains("Critical CVE found in openssl"));
        assert!(summary.contains("Action Items:"));
    }

    #[test]
    fn test_generate_digest_prompt_contains_stats() {
        let data = DigestData {
            new_rfcs: 5,
            health_score: 87.5,
            health_delta: 2.3,
            ..Default::default()
        };
        let prompt = generate_digest_prompt(&data);
        assert!(prompt.contains("New RFCs: 5"));
        assert!(prompt.contains("87.5"));
        assert!(prompt.contains("+2.3"));
    }
}
