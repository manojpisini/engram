//! Code pattern extraction and frequency tracking.
//!
//! Identifies recurring patterns across PR reviews and tracks their trend
//! (Increasing / Stable / Decreasing) over time.

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::pr_analyzer::ReviewResult;

// ─── Data structures ────────────────────────────────────────────────

/// A pattern match extracted from a single review.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternMatch {
    pub pattern_name: String,
    pub category: String,
    pub severity: String,
    pub occurrences: u32,
    pub example_files: Vec<String>,
}

/// Trend direction for a pattern's frequency over time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Trend {
    Increasing,
    Stable,
    Decreasing,
}

impl std::fmt::Display for Trend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Trend::Increasing => write!(f, "Increasing"),
            Trend::Stable => write!(f, "Stable"),
            Trend::Decreasing => write!(f, "Decreasing"),
        }
    }
}

/// Accumulated frequency data for a pattern (across multiple PRs).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatternFrequency {
    pub pattern_name: String,
    pub category: String,
    pub total_frequency: u32,
    pub trend: Trend,
    pub last_seen_pr: Option<u64>,
}

// ─── Extraction logic ───────────────────────────────────────────────

/// Extract pattern matches from a review result.
///
/// Collects `patterns_observed` from the Claude review and also scans findings
/// for files that appear in multiple categories (blocker + suggestion on the
/// same file, etc.) to detect cross-cutting patterns.
pub fn extract_patterns(review: &ReviewResult) -> Vec<PatternMatch> {
    let mut patterns: Vec<PatternMatch> = Vec::new();

    // 1. Direct patterns from Claude's analysis
    for obs in &review.patterns_observed {
        let example_files = collect_example_files(review, &obs.pattern_name);
        patterns.push(PatternMatch {
            pattern_name: obs.pattern_name.clone(),
            category: obs.category.clone(),
            severity: obs.severity.clone(),
            occurrences: obs.occurrences,
            example_files,
        });
    }

    // 2. Infer patterns from rule_id clusters — if the same rule_id appears
    //    in 2+ findings across different files, that is a pattern.
    let mut rule_counts: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();

    let all_findings = review
        .blockers
        .iter()
        .chain(review.suggestions.iter())
        .chain(review.nits.iter());

    for finding in all_findings {
        rule_counts
            .entry(finding.rule_id.clone())
            .or_default()
            .push(finding.file.clone());
    }

    for (rule_id, files) in &rule_counts {
        if files.len() >= 2 {
            // Only add if not already captured by Claude's patterns_observed
            let already_captured = patterns.iter().any(|p| p.pattern_name == *rule_id);
            if !already_captured {
                let unique_files: Vec<String> = {
                    let mut f = files.clone();
                    f.sort();
                    f.dedup();
                    f
                };
                patterns.push(PatternMatch {
                    pattern_name: format!("rule-cluster-{rule_id}"),
                    category: "inferred".into(),
                    severity: "SUGGESTION".into(),
                    occurrences: files.len() as u32,
                    example_files: unique_files,
                });
            }
        }
    }

    patterns
}

/// Collect file names from findings whose descriptions or rule_ids mention the
/// given pattern name.
fn collect_example_files(review: &ReviewResult, pattern_name: &str) -> Vec<String> {
    let all_findings = review
        .blockers
        .iter()
        .chain(review.suggestions.iter())
        .chain(review.nits.iter());

    let mut files: Vec<String> = all_findings
        .filter(|f| {
            f.rule_id == pattern_name
                || f.title.contains(pattern_name)
                || f.description.contains(pattern_name)
        })
        .map(|f| f.file.clone())
        .collect();
    files.sort();
    files.dedup();
    files
}

/// Update a pattern's frequency count in the Review Patterns DB.
///
/// In production this calls the Notion MCP tool to read the current record,
/// increment frequency, and write it back. For now we log the intent.
pub async fn update_pattern_frequency(
    pattern: &PatternMatch,
    pr_number: u64,
) -> anyhow::Result<PatternFrequency> {
    info!(
        "[ReviewAgent] Would call MCP: notion_update_page with \
         {{\"database\": \"ENGRAM/Review Patterns\", \"pattern_name\": \"{}\", \
         \"frequency\": \"+{}\", \"last_seen_pr\": {}}}",
        pattern.pattern_name, pattern.occurrences, pr_number
    );

    // Placeholder: pretend the new total is the current occurrences.
    Ok(PatternFrequency {
        pattern_name: pattern.pattern_name.clone(),
        category: pattern.category.clone(),
        total_frequency: pattern.occurrences,
        trend: Trend::Stable,
        last_seen_pr: Some(pr_number),
    })
}

/// Detect a trend given a short window of recent frequency values.
///
/// - If the last 3 values are strictly increasing → `Increasing`
/// - If the last 3 values are strictly decreasing → `Decreasing`
/// - Otherwise → `Stable`
pub fn detect_trend(recent_frequencies: &[u32]) -> Trend {
    if recent_frequencies.len() < 3 {
        return Trend::Stable;
    }
    let window = &recent_frequencies[recent_frequencies.len() - 3..];
    if window[0] < window[1] && window[1] < window[2] {
        Trend::Increasing
    } else if window[0] > window[1] && window[1] > window[2] {
        Trend::Decreasing
    } else {
        Trend::Stable
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pr_analyzer::{ObservedPattern, ReviewFinding, ReviewResult};

    fn make_finding(rule_id: &str, file: &str) -> ReviewFinding {
        ReviewFinding {
            rule_id: rule_id.into(),
            file: file.into(),
            line: None,
            title: "test".into(),
            description: "test desc".into(),
            suggested_fix: None,
        }
    }

    #[test]
    fn test_extract_patterns_from_observed() {
        let review = ReviewResult {
            blockers: Vec::new(),
            suggestions: Vec::new(),
            nits: Vec::new(),
            quality_score: 80,
            quality_rationale: None,
            patterns_observed: vec![ObservedPattern {
                pattern_name: "unwrap-usage".into(),
                category: "error-handling".into(),
                occurrences: 5,
                severity: "SUGGESTION".into(),
            }],
            summary: None,
        };
        let patterns = extract_patterns(&review);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].pattern_name, "unwrap-usage");
    }

    #[test]
    fn test_extract_inferred_patterns() {
        let review = ReviewResult {
            blockers: Vec::new(),
            suggestions: vec![
                make_finding("RULE-010", "src/a.rs"),
                make_finding("RULE-010", "src/b.rs"),
                make_finding("RULE-010", "src/c.rs"),
            ],
            nits: Vec::new(),
            quality_score: 60,
            quality_rationale: None,
            patterns_observed: Vec::new(),
            summary: None,
        };
        let patterns = extract_patterns(&review);
        assert_eq!(patterns.len(), 1);
        assert!(patterns[0].pattern_name.contains("RULE-010"));
        assert_eq!(patterns[0].occurrences, 3);
    }

    #[test]
    fn test_detect_trend() {
        assert_eq!(detect_trend(&[1, 2, 3]), Trend::Increasing);
        assert_eq!(detect_trend(&[5, 3, 1]), Trend::Decreasing);
        assert_eq!(detect_trend(&[2, 5, 3]), Trend::Stable);
        assert_eq!(detect_trend(&[1, 2]), Trend::Stable); // too short
    }
}
