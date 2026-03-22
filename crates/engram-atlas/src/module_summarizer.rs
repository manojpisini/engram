//! Module documentation generation and Claude response parsing.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::prompts;

/// Structured summary of a code module, produced by Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleSummary {
    /// 2-3 sentence plain-English summary of the module's purpose.
    pub what_it_does: String,
    /// Key structs, traits, or concepts in this module.
    pub main_abstractions: Vec<String>,
    /// Primary public functions/endpoints a developer calls first.
    pub entry_points: Vec<String>,
    /// Non-obvious pitfalls, ordering constraints, or footguns.
    pub common_gotchas: Vec<String>,
    /// Cognitive complexity rating 1-10.
    pub complexity_score: u8,
    /// Single sentence justifying the complexity score.
    pub complexity_reasoning: String,
}

/// Parse a Claude JSON response into a [`ModuleSummary`].
///
/// The response may contain markdown code fences; they are stripped before parsing.
pub fn parse_module_summary(claude_response: &str) -> Result<ModuleSummary> {
    let json_str = extract_json_block(claude_response);
    let summary: ModuleSummary =
        serde_json::from_str(json_str).context("Failed to parse module summary JSON from Claude response")?;

    // Clamp complexity score to valid range
    let summary = ModuleSummary {
        complexity_score: summary.complexity_score.clamp(1, 10),
        ..summary
    };

    info!(
        "[ModuleSummarizer] Parsed summary: complexity={}, abstractions={}",
        summary.complexity_score,
        summary.main_abstractions.len()
    );

    Ok(summary)
}

/// Build the prompt for module summarization. Delegates to [`prompts::module_summarization_prompt`].
pub fn build_summarization_prompt(
    module_name: &str,
    module_path: &str,
    key_files: &[String],
    diff_context: Option<&str>,
) -> String {
    info!(
        "[ModuleSummarizer] Building summarization prompt for module={module_name} path={module_path}"
    );
    prompts::module_summarization_prompt(module_name, module_path, key_files, diff_context)
}

/// Extract the JSON body from a Claude response that may be wrapped in markdown fences.
///
/// Handles both ````json ... ```` and bare JSON.
fn extract_json_block(text: &str) -> &str {
    let trimmed = text.trim();

    // Try to find ```json ... ``` block
    if let Some(start) = trimmed.find("```json") {
        let after_fence = &trimmed[start + 7..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim();
        }
    }

    // Try to find ``` ... ``` block
    if let Some(start) = trimmed.find("```") {
        let after_fence = &trimmed[start + 3..];
        if let Some(end) = after_fence.find("```") {
            return after_fence[..end].trim();
        }
    }

    // Assume the whole thing is JSON
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_module_summary_bare_json() {
        let response = r#"{
            "what_it_does": "Handles user auth.",
            "main_abstractions": ["AuthService", "TokenStore"],
            "entry_points": ["login()", "verify_token()"],
            "common_gotchas": ["Tokens expire after 1h"],
            "complexity_score": 4,
            "complexity_reasoning": "Standard OAuth2 flow with some edge cases."
        }"#;
        let summary = parse_module_summary(response).unwrap();
        assert_eq!(summary.complexity_score, 4);
        assert_eq!(summary.main_abstractions.len(), 2);
        assert_eq!(summary.entry_points.len(), 2);
    }

    #[test]
    fn test_parse_module_summary_with_fences() {
        let response = r#"Here is the summary:
```json
{
    "what_it_does": "Manages configs.",
    "main_abstractions": ["Config"],
    "entry_points": ["load()"],
    "common_gotchas": [],
    "complexity_score": 2,
    "complexity_reasoning": "Simple key-value store."
}
```"#;
        let summary = parse_module_summary(response).unwrap();
        assert_eq!(summary.complexity_score, 2);
        assert_eq!(summary.what_it_does, "Manages configs.");
    }

    #[test]
    fn test_complexity_score_clamped() {
        let response = r#"{
            "what_it_does": "Test.",
            "main_abstractions": [],
            "entry_points": [],
            "common_gotchas": [],
            "complexity_score": 15,
            "complexity_reasoning": "Over the top."
        }"#;
        let summary = parse_module_summary(response).unwrap();
        assert_eq!(summary.complexity_score, 10);
    }
}
