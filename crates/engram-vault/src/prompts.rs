//! Claude prompt templates for the Vault agent.
//!
//! These templates are used to invoke Claude for env-var analysis tasks:
//! detecting missing vars, rotation issues, sensitivity mismatches, and orphaned vars.

use crate::env_differ::EnvDiffResult;
use crate::env_parser::EnvVarRef;

/// Categories of issues the env diff analysis prompt can surface.
pub const ISSUE_MISSING_IN_PROD: &str = "MISSING_IN_PROD";
pub const ISSUE_ROTATION_OVERDUE: &str = "ROTATION_OVERDUE";
pub const ISSUE_SENSITIVITY_MISMATCHES: &str = "SENSITIVITY_MISMATCHES";
pub const ISSUE_ORPHANED_VARS: &str = "ORPHANED_VARS";

/// Build the environment diff analysis prompt for Claude.
///
/// Given a diff result, produces a prompt asking Claude to analyse the
/// cross-environment state and produce prioritised recommendations.
pub fn env_diff_analysis_prompt(project_name: &str, diff: &EnvDiffResult) -> String {
    let missing_prod_list = if diff.missing_in_prod.is_empty() {
        "  (none)".to_string()
    } else {
        diff.missing_in_prod
            .iter()
            .map(|v| format!("  - {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let missing_staging_list = if diff.missing_in_staging.is_empty() {
        "  (none)".to_string()
    } else {
        diff.missing_in_staging
            .iter()
            .map(|v| format!("  - {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mismatch_list = if diff.mismatches.is_empty() {
        "  (none)".to_string()
    } else {
        diff.mismatches
            .iter()
            .map(|m| format!("  - {}: {}", m.var_name, m.detail))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let orphaned_list = if diff.orphaned_in_prod.is_empty() {
        "  (none)".to_string()
    } else {
        diff.orphaned_in_prod
            .iter()
            .map(|v| format!("  - {v}"))
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        r#"You are the ENGRAM Vault agent analysing environment variable configuration for project "{project_name}".

## Current State
Total variables tracked: {total}

### {ISSUE_MISSING_IN_PROD} — Variables missing in production
{missing_prod_list}

### {ISSUE_MISSING_IN_PROD} (staging) — Variables missing in staging
{missing_staging_list}

### {ISSUE_SENSITIVITY_MISMATCHES} — Sensitivity classification differs across environments
{mismatch_list}

### {ISSUE_ORPHANED_VARS} — Variables in prod/staging but not referenced in dev
{orphaned_list}

## Instructions
1. For each {ISSUE_MISSING_IN_PROD} variable, assess the risk level (Critical / High / Medium / Low).
   - Variables that look like secrets (containing KEY, SECRET, TOKEN, PASSWORD, CREDENTIAL) are Critical.
   - Database connection strings are High.
   - Feature flags and non-sensitive config are Medium/Low.

2. For each {ISSUE_SENSITIVITY_MISMATCHES}, recommend the correct classification and which environments need updating.

3. For each {ISSUE_ORPHANED_VARS}, determine if the variable is likely still needed or can be safely removed.
   Suggest a cleanup action with a confidence level.

4. Produce a prioritised action list, ordered by severity, with:
   - Action type: "add_to_prod" | "add_to_staging" | "update_sensitivity" | "remove_orphan" | "investigate"
   - Variable name
   - Severity: Critical | High | Medium | Low
   - Recommended owner (guess from variable naming: DB vars -> backend, UI vars -> frontend, etc.)
   - Brief rationale (one sentence)

Return your analysis as JSON with this schema:
{{
  "summary": "one-paragraph overview",
  "actions": [
    {{
      "action_type": "string",
      "var_name": "string",
      "severity": "string",
      "recommended_owner": "string",
      "rationale": "string"
    }}
  ],
  "risk_score": 0-100
}}"#,
        total = diff.total_vars,
    )
}

/// Build a prompt for analysing newly detected env vars from a PR diff.
///
/// Used when a `PrMerged` event introduces new environment variable references.
pub fn new_env_vars_analysis_prompt(
    project_name: &str,
    pr_number: u64,
    pr_title: &str,
    env_vars: &[EnvVarRef],
) -> String {
    let var_list = env_vars
        .iter()
        .map(|v| format!("  - {} (file: {}, line: {})", v.name, v.source_file, v.line))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are the ENGRAM Vault agent. PR #{pr_number} ("{pr_title}") in project "{project_name}" introduced new environment variable references:

{var_list}

For each variable, determine:
1. **Type**: Secret | API Key | Database URL | Config | Feature Flag | Other
2. **Sensitivity**: Secret (must be encrypted/rotated) | Internal (not public but not a secret) | Public (safe to commit)
3. **Suggested rotation policy**: "30d" for secrets, "90d" for API keys, "never" for config/flags
4. **Required in environments**: Which of dev, staging, prod, CI need this variable?
5. **Description**: One-sentence description of what this variable controls.

Return your analysis as JSON:
{{
  "variables": [
    {{
      "name": "string",
      "var_type": "string",
      "sensitivity": "string",
      "rotation_policy": "string",
      "required_environments": ["dev", "staging", "prod", "ci"],
      "description": "string"
    }}
  ]
}}"#
    )
}

/// Build a prompt for rotation-overdue secrets.
///
/// Used during the daily rotation check to get Claude's prioritisation of
/// which overdue secrets are most critical.
pub fn rotation_overdue_prompt(
    project_name: &str,
    overdue_vars: &[(String, i64)], // (var_name, days_overdue)
) -> String {
    let var_list = overdue_vars
        .iter()
        .map(|(name, days)| format!("  - {name}: {days} days overdue"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are the ENGRAM Vault agent. The following secrets in project "{project_name}" are past their rotation deadline:

### {ISSUE_ROTATION_OVERDUE}
{var_list}

Prioritise these by risk. Consider:
- Secrets overdue by more than 2x their policy period are Critical.
- API keys and tokens are higher risk than config values.
- Variables with "PROD" or "PRODUCTION" in their name are higher priority.

Return JSON:
{{
  "priority_order": [
    {{
      "var_name": "string",
      "severity": "Critical | High | Medium | Low",
      "days_overdue": number,
      "recommendation": "string"
    }}
  ],
  "overall_risk": "Critical | High | Medium | Low"
}}"#
    )
}

/// Build a prompt for RFC-required env var scaffolding.
///
/// Used when an RFC is approved and lists required environment variables.
pub fn rfc_env_scaffold_prompt(
    project_name: &str,
    rfc_id: &str,
    required_vars: &[String],
) -> String {
    let var_list = required_vars
        .iter()
        .map(|v| format!("  - {v}"))
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"You are the ENGRAM Vault agent. RFC "{rfc_id}" in project "{project_name}" has been approved and requires these environment variables:

{var_list}

For each variable, suggest:
1. **Type**: Secret | API Key | Database URL | Config | Feature Flag
2. **Sensitivity**: Secret | Internal | Public
3. **Rotation policy**: appropriate rotation period
4. **Environments needed**: which of dev, staging, prod, CI
5. **Description**: what this variable is for based on its name

Return JSON:
{{
  "scaffolded_vars": [
    {{
      "name": "string",
      "var_type": "string",
      "sensitivity": "string",
      "rotation_policy": "string",
      "environments": ["dev", "staging", "prod", "ci"],
      "description": "string"
    }}
  ]
}}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env_differ::{EnvDiffResult, EnvMismatch};
    use chrono::Utc;

    #[test]
    fn test_env_diff_prompt_contains_sections() {
        let diff = EnvDiffResult {
            missing_in_staging: vec!["API_KEY".to_string()],
            missing_in_prod: vec!["DATABASE_URL".to_string()],
            staging_not_in_prod: vec![],
            mismatches: vec![EnvMismatch {
                var_name: "TOKEN".to_string(),
                detail: "sensitivity mismatch".to_string(),
            }],
            orphaned_in_prod: vec!["OLD_VAR".to_string()],
            total_vars: 5,
            snapshot_at: Utc::now(),
        };

        let prompt = env_diff_analysis_prompt("my-project", &diff);
        assert!(prompt.contains("MISSING_IN_PROD"));
        assert!(prompt.contains("DATABASE_URL"));
        assert!(prompt.contains("SENSITIVITY_MISMATCHES"));
        assert!(prompt.contains("TOKEN"));
        assert!(prompt.contains("ORPHANED_VARS"));
        assert!(prompt.contains("OLD_VAR"));
    }

    #[test]
    fn test_new_env_vars_prompt() {
        let vars = vec![EnvVarRef {
            name: "SECRET_KEY".to_string(),
            source_file: "src/config.rs".to_string(),
            line: 10,
        }];
        let prompt = new_env_vars_analysis_prompt("proj", 42, "Add auth", &vars);
        assert!(prompt.contains("SECRET_KEY"));
        assert!(prompt.contains("PR #42"));
    }

    #[test]
    fn test_rotation_overdue_prompt() {
        let overdue = vec![("DB_PASSWORD".to_string(), 15)];
        let prompt = rotation_overdue_prompt("proj", &overdue);
        assert!(prompt.contains("DB_PASSWORD"));
        assert!(prompt.contains("15 days overdue"));
        assert!(prompt.contains("ROTATION_OVERDUE"));
    }
}
