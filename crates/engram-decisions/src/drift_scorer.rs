use anyhow::{Context, Result};
use serde::Deserialize;
use tracing::info;

/// Parsed result from Claude's Decision Rationale response.
#[derive(Debug, Clone, Deserialize)]
pub struct DriftAnalysis {
    pub decision_rationale: String,
    pub drift_score: u8,
    pub drift_notes: String,
}

impl DriftAnalysis {
    /// Parse Claude's JSON response into a DriftAnalysis.
    ///
    /// Validates that `drift_score` is in the range 0..=10.
    pub fn from_claude_response(raw_json: &str) -> Result<Self> {
        let analysis: DriftAnalysis = serde_json::from_str(raw_json)
            .context("Failed to parse Claude drift analysis JSON")?;

        anyhow::ensure!(
            analysis.drift_score <= 10,
            "Drift score {} is out of range 0-10",
            analysis.drift_score
        );

        info!(
            "[DecisionsAgent] Drift analysis: score={}, rationale_len={}, notes_len={}",
            analysis.drift_score,
            analysis.decision_rationale.len(),
            analysis.drift_notes.len()
        );

        Ok(analysis)
    }
}

/// Write drift analysis results to the RFC Notion page.
///
/// Currently logs what it would do; actual MCP calls will be wired up later.
pub async fn write_drift_to_notion(
    rfc_page_id: &str,
    analysis: &DriftAnalysis,
) -> Result<()> {
    info!(
        "[DecisionsAgent] Would call MCP: notion.update_page with {{ page_id: \"{rfc_page_id}\", \
         properties: {{ \"Decision Rationale\": \"{rationale}\", \"Drift Score\": {score}, \
         \"Drift Notes\": \"{notes}\" }} }}",
        rationale = truncate(&analysis.decision_rationale, 80),
        score = analysis.drift_score,
        notes = truncate(&analysis.drift_notes, 80),
    );
    Ok(())
}

/// Truncate a string for logging purposes.
fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_response() {
        let json = r#"{
            "decision_rationale": "Implementation matches the RFC closely.",
            "drift_score": 2,
            "drift_notes": "Minor deviation in error handling approach."
        }"#;
        let analysis = DriftAnalysis::from_claude_response(json).unwrap();
        assert_eq!(analysis.drift_score, 2);
        assert!(!analysis.decision_rationale.is_empty());
    }

    #[test]
    fn reject_out_of_range_score() {
        let json = r#"{
            "decision_rationale": "Totally different.",
            "drift_score": 15,
            "drift_notes": "Everything diverged."
        }"#;
        let result = DriftAnalysis::from_claude_response(json);
        assert!(result.is_err());
    }

    #[test]
    fn reject_malformed_json() {
        let result = DriftAnalysis::from_claude_response("not json at all");
        assert!(result.is_err());
    }
}
