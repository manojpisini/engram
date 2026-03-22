//! Claude prompt templates for the Release agent.

/// Generate release notes from merged PRs and implemented RFCs.
pub fn release_notes_prompt(
    version: &str,
    project_name: &str,
    merged_prs_json: &str,
    implemented_rfcs_json: &str,
    dependency_changes_json: &str,
) -> String {
    format!(
        r#"You are a technical writer generating release notes for version {version} of {project_name}.

Merged PRs in this release:
<prs>{merged_prs_json}</prs>

Implemented RFCs in this release:
<rfcs>{implemented_rfcs_json}</rfcs>

Dependency changes:
<deps>{dependency_changes_json}</deps>

Generate user-facing release notes grouped into these sections:
1. FEATURES: New capabilities added (from RFCs and feature PRs)
2. FIXES: Bug fixes
3. PERFORMANCE: Performance improvements or changes
4. SECURITY: Security fixes, dependency updates addressing CVEs
5. BREAKING_CHANGES: Any breaking changes requiring user action

For each item, write a concise one-line description with the PR reference.

Format as JSON:
{{
  "features": ["..."],
  "fixes": ["..."],
  "performance": ["..."],
  "security": ["..."],
  "breaking_changes": ["..."],
  "summary": "1-2 sentence release summary"
}}"#
    )
}

/// Generate migration notes for breaking changes and new config requirements.
pub fn migration_notes_prompt(
    version: &str,
    breaking_changes_json: &str,
    new_env_vars_json: &str,
    dependency_changes_json: &str,
) -> String {
    format!(
        r#"You are a DevOps engineer writing migration/upgrade notes for version {version}.

Breaking changes:
<breaking>{breaking_changes_json}</breaking>

New environment variables required:
<env_vars>{new_env_vars_json}</env_vars>

Dependency changes:
<deps>{dependency_changes_json}</deps>

Generate step-by-step migration notes that an engineer can follow to upgrade:
1. BEFORE_UPGRADE: Steps to take before upgrading (backups, config changes)
2. ENV_CHANGES: New env vars to add, with descriptions and example values
3. BREAKING_MIGRATION: For each breaking change, the exact steps to adapt
4. DEPENDENCY_NOTES: Any manual dependency actions needed
5. VERIFICATION: How to verify the upgrade succeeded

Format as JSON:
{{
  "before_upgrade": ["..."],
  "env_changes": [{{"var": "...", "description": "...", "example": "..."}}],
  "breaking_migration": [{{"change": "...", "steps": ["..."]}}],
  "dependency_notes": ["..."],
  "verification": ["..."]
}}"#
    )
}

/// Generate a release readiness assessment.
pub fn release_readiness_prompt(
    version: &str,
    open_regressions_json: &str,
    unresolved_cves_json: &str,
    health_scores_json: &str,
) -> String {
    format!(
        r#"You are a release manager assessing whether version {version} is ready to ship.

Open regressions:
<regressions>{open_regressions_json}</regressions>

Unresolved CVEs (Critical/High):
<cves>{unresolved_cves_json}</cves>

Current health scores:
<health>{health_scores_json}</health>

Assess:
1. RELEASE_READY: true/false
2. BLOCKERS: List any blocking issues that must be resolved
3. RISKS: Non-blocking risks to note
4. RECOMMENDATION: Ship / Ship with caveats / Hold with reasoning

Format as JSON:
{{
  "release_ready": true|false,
  "blockers": ["..."],
  "risks": ["..."],
  "recommendation": "..."
}}"#
    )
}
