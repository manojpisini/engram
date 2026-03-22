//! Claude prompt templates for the Review agent.

/// Build the PR review generation prompt.
///
/// This prompt instructs Claude to analyze a PR diff against the project's
/// Review Playbook rules and produce a structured review with categories
/// BLOCKER / SUGGESTION / NIT plus a quality score and observed patterns.
pub fn pr_review_prompt(
    diff: &str,
    playbook_rules_json: &str,
    pr_title: &str,
    pr_description: &str,
    author: &str,
    branch: &str,
    target_branch: &str,
) -> String {
    format!(
        r#"You are ENGRAM Review, an AI code reviewer.

## Context
- **PR Title:** {pr_title}
- **Author:** {author}
- **Branch:** {branch} → {target_branch}
- **Description:** {pr_description}

## Active Review Playbook Rules
```json
{playbook_rules_json}
```

## PR Diff
```diff
{diff}
```

## Instructions
Analyze the diff against every active playbook rule. For each finding, classify it as one of:
- **BLOCKER** — Must be fixed before merge. Security issues, data loss risks, correctness bugs.
- **SUGGESTION** — Should be fixed; improves quality. Performance, maintainability, readability.
- **NIT** — Optional polish. Style, naming, minor cleanups.

## Output Format
Return valid JSON (no markdown fence) with this exact structure:
{{
  "blockers": [
    {{
      "rule_id": "<playbook rule id>",
      "file": "<file path>",
      "line": <line number or null>,
      "title": "<short title>",
      "description": "<detailed explanation>",
      "suggested_fix": "<code or guidance>"
    }}
  ],
  "suggestions": [
    {{
      "rule_id": "<playbook rule id>",
      "file": "<file path>",
      "line": <line number or null>,
      "title": "<short title>",
      "description": "<detailed explanation>",
      "suggested_fix": "<code or guidance>"
    }}
  ],
  "nits": [
    {{
      "rule_id": "<playbook rule id>",
      "file": "<file path>",
      "line": <line number or null>,
      "title": "<short title>",
      "description": "<detailed explanation>"
    }}
  ],
  "quality_score": <0-100>,
  "quality_rationale": "<brief rationale for the score>",
  "patterns_observed": [
    {{
      "pattern_name": "<e.g. missing-error-handling, unwrap-usage>",
      "category": "<category from playbook>",
      "occurrences": <count>,
      "severity": "BLOCKER|SUGGESTION|NIT"
    }}
  ],
  "summary": "<2-3 sentence overall review summary>"
}}"#
    )
}

/// Build a prompt to extract recurring code patterns from a review result.
pub fn pattern_extraction_prompt(review_json: &str) -> String {
    format!(
        r#"You are ENGRAM Pattern Analyzer.

Given the following PR review result, identify recurring code patterns
that appear across multiple findings. Group similar issues together and
name each pattern concisely.

## Review Result
```json
{review_json}
```

## Output Format
Return valid JSON (no markdown fence):
{{
  "patterns": [
    {{
      "pattern_name": "<concise-kebab-case-name>",
      "category": "<error-handling|security|performance|style|correctness|testing>",
      "description": "<what the pattern is>",
      "severity": "BLOCKER|SUGGESTION|NIT",
      "occurrence_count": <number>,
      "example_files": ["<file1>", "<file2>"]
    }}
  ]
}}"#
    )
}

/// Build a prompt to generate a tech debt item description from a recurring pattern.
pub fn tech_debt_promotion_prompt(
    pattern_name: &str,
    frequency: u32,
    threshold: u32,
    category: &str,
) -> String {
    format!(
        r#"You are ENGRAM Tech Debt Tracker.

A review pattern "{pattern_name}" (category: {category}) has been observed
{frequency} times, exceeding the promotion threshold of {threshold}.

Generate a concise tech debt item.

## Output Format
Return valid JSON (no markdown fence):
{{
  "debt_title": "<actionable title>",
  "description": "<what needs to be done and why>",
  "severity": "Critical|High|Medium|Low",
  "effort_estimate": "<e.g. 2h, 1d, 3d, 1w>",
  "suggested_approach": "<how to address this debt>"
}}"#
    )
}
