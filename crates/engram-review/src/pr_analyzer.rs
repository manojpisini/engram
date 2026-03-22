//! PR analysis logic — parse diffs against playbook rules and produce structured reviews.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::prompts;

// ─── Data structures ────────────────────────────────────────────────

/// A single review finding (BLOCKER, SUGGESTION, or NIT).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewFinding {
    pub rule_id: String,
    pub file: String,
    pub line: Option<u64>,
    pub title: String,
    pub description: String,
    #[serde(default)]
    pub suggested_fix: Option<String>,
}

/// An observed code pattern found during review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedPattern {
    pub pattern_name: String,
    pub category: String,
    pub occurrences: u32,
    pub severity: String,
}

/// The full result of analyzing a PR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub blockers: Vec<ReviewFinding>,
    pub suggestions: Vec<ReviewFinding>,
    pub nits: Vec<ReviewFinding>,
    pub quality_score: u32,
    #[serde(default)]
    pub quality_rationale: Option<String>,
    pub patterns_observed: Vec<ObservedPattern>,
    #[serde(default)]
    pub summary: Option<String>,
}

/// Metadata about the PR being analyzed (for logging / record creation).
#[derive(Debug, Clone)]
pub struct PrContext {
    pub repo: String,
    pub pr_number: u64,
    pub title: String,
    pub description: String,
    pub author: String,
    pub branch: String,
    pub target_branch: String,
}

// ─── Analysis entry point ───────────────────────────────────────────

/// Analyze a PR diff against the supplied playbook rules.
///
/// In production this sends the prompt to Claude and parses the JSON response.
/// For now it logs the call and returns a placeholder `ReviewResult`.
pub async fn analyze_pr(
    diff: &str,
    playbook_rules_json: &str,
    ctx: &PrContext,
) -> anyhow::Result<ReviewResult> {
    let prompt = prompts::pr_review_prompt(
        diff,
        playbook_rules_json,
        &ctx.title,
        &ctx.description,
        &ctx.author,
        &ctx.branch,
        &ctx.target_branch,
    );

    info!(
        "[ReviewAgent] Would call Claude for: PR review of #{} ({}) — prompt length {} chars",
        ctx.pr_number,
        ctx.title,
        prompt.len()
    );

    // In production: send `prompt` to Claude, receive JSON, then call
    // `parse_review_response`. For now, return a placeholder.
    Ok(ReviewResult {
        blockers: Vec::new(),
        suggestions: Vec::new(),
        nits: Vec::new(),
        quality_score: 0,
        quality_rationale: Some("Placeholder — Claude not called yet".into()),
        patterns_observed: Vec::new(),
        summary: Some(format!(
            "Placeholder review for PR #{} ({})",
            ctx.pr_number, ctx.title
        )),
    })
}

/// Parse Claude's JSON response into a `ReviewResult`.
///
/// Returns an error if the JSON does not match the expected schema.
pub fn parse_review_response(json_str: &str) -> anyhow::Result<ReviewResult> {
    let result: ReviewResult = serde_json::from_str(json_str)?;
    Ok(result)
}

/// Format the review result into a human-readable Markdown draft suitable
/// for writing into the Notion PR Review record's "Claude Review Draft" field.
pub fn format_review_draft(result: &ReviewResult) -> String {
    let mut out = String::new();

    out.push_str("# ENGRAM Code Review Draft\n\n");

    if !result.blockers.is_empty() {
        out.push_str(&format!("## BLOCKERS ({})\n\n", result.blockers.len()));
        for (i, b) in result.blockers.iter().enumerate() {
            out.push_str(&format!(
                "{}. **{}** (`{}` {})\n   Rule: `{}`\n   {}\n",
                i + 1,
                b.title,
                b.file,
                b.line.map_or(String::new(), |l| format!("L{l}")),
                b.rule_id,
                b.description
            ));
            if let Some(fix) = &b.suggested_fix {
                out.push_str(&format!("   **Fix:** {fix}\n"));
            }
            out.push('\n');
        }
    }

    if !result.suggestions.is_empty() {
        out.push_str(&format!(
            "## SUGGESTIONS ({})\n\n",
            result.suggestions.len()
        ));
        for (i, s) in result.suggestions.iter().enumerate() {
            out.push_str(&format!(
                "{}. **{}** (`{}` {})\n   Rule: `{}`\n   {}\n",
                i + 1,
                s.title,
                s.file,
                s.line.map_or(String::new(), |l| format!("L{l}")),
                s.rule_id,
                s.description
            ));
            if let Some(fix) = &s.suggested_fix {
                out.push_str(&format!("   **Fix:** {fix}\n"));
            }
            out.push('\n');
        }
    }

    if !result.nits.is_empty() {
        out.push_str(&format!("## NITS ({})\n\n", result.nits.len()));
        for (i, n) in result.nits.iter().enumerate() {
            out.push_str(&format!(
                "{}. **{}** (`{}` {})\n   Rule: `{}`\n   {}\n\n",
                i + 1,
                n.title,
                n.file,
                n.line.map_or(String::new(), |l| format!("L{l}")),
                n.rule_id,
                n.description
            ));
        }
    }

    out.push_str(&format!(
        "---\n**Quality Score:** {}/100\n",
        result.quality_score
    ));
    if let Some(rationale) = &result.quality_rationale {
        out.push_str(&format!("**Rationale:** {rationale}\n"));
    }
    if let Some(summary) = &result.summary {
        out.push_str(&format!("\n**Summary:** {summary}\n"));
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_review_response() {
        let json = r#"{
            "blockers": [],
            "suggestions": [{
                "rule_id": "RULE-001",
                "file": "src/main.rs",
                "line": 42,
                "title": "Missing error handling",
                "description": "unwrap() used on fallible operation",
                "suggested_fix": "Use ? operator or match"
            }],
            "nits": [],
            "quality_score": 75,
            "quality_rationale": "Generally good but needs error handling",
            "patterns_observed": [{
                "pattern_name": "unwrap-usage",
                "category": "error-handling",
                "occurrences": 3,
                "severity": "SUGGESTION"
            }],
            "summary": "Good PR with minor issues"
        }"#;
        let result = parse_review_response(json).unwrap();
        assert_eq!(result.quality_score, 75);
        assert_eq!(result.suggestions.len(), 1);
        assert_eq!(result.patterns_observed.len(), 1);
        assert_eq!(result.patterns_observed[0].pattern_name, "unwrap-usage");
    }

    #[test]
    fn test_format_review_draft() {
        let result = ReviewResult {
            blockers: vec![ReviewFinding {
                rule_id: "SEC-001".into(),
                file: "src/auth.rs".into(),
                line: Some(10),
                title: "SQL Injection".into(),
                description: "Unsanitized input".into(),
                suggested_fix: Some("Use parameterized queries".into()),
            }],
            suggestions: Vec::new(),
            nits: Vec::new(),
            quality_score: 40,
            quality_rationale: Some("Critical security issue".into()),
            patterns_observed: Vec::new(),
            summary: Some("Needs security fixes".into()),
        };
        let draft = format_review_draft(&result);
        assert!(draft.contains("BLOCKERS (1)"));
        assert!(draft.contains("SQL Injection"));
        assert!(draft.contains("40/100"));
    }
}
