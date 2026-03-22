//! Notion database field name constants for all 16 databases.
//! Every DB operation must use Notion MCP tools — never the raw Notion REST API.

// ─── Anchor ───

pub mod projects {
    pub const DB_NAME: &str = "ENGRAM/Projects";
    pub const NAME: &str = "Name";
    pub const DESCRIPTION: &str = "Description";
    pub const REPO_URL: &str = "Repo URL";
    pub const STATUS: &str = "Status";
    pub const CREATED_AT: &str = "Created At";
}

// ─── ENGRAM/Decisions ───

pub mod rfcs {
    pub const DB_NAME: &str = "ENGRAM/RFCs";
    pub const RFC_ID: &str = "RFC ID";
    pub const TITLE: &str = "RFC Title";
    pub const STATUS: &str = "Status";
    pub const AUTHOR: &str = "Author";
    pub const PROBLEM_STATEMENT: &str = "Problem Statement";
    pub const PROPOSED_SOLUTION: &str = "Proposed Solution";
    pub const AFFECTED_MODULES: &str = "Affected Modules";
    pub const REQUIRED_ENV_VARS: &str = "Env Vars";
    pub const BANNED_PATTERNS: &str = "Banned Patterns";
    pub const PROJECT: &str = "Project ID";
    pub const ALTERNATIVES_CONSIDERED: &str = "Alternatives Considered";
    pub const TRADE_OFFS: &str = "Trade-offs";
    pub const DECISION_RATIONALE: &str = "Decision Rationale";
    pub const REVIEWERS: &str = "Reviewers";
    pub const IMPLEMENTATION_PRS: &str = "Implementation PRs";
    pub const AFFECTED_DEPENDENCIES: &str = "Affected Dependencies";
    pub const BENCHMARK_BASELINE: &str = "Benchmark Baseline";
    pub const POST_RFC_BENCHMARKS: &str = "Post-RFC Benchmarks";
    pub const OPENS: &str = "Opens";
    pub const RESOLVES: &str = "Resolves";
    pub const DRIFT_NOTES: &str = "Drift Notes";
    pub const RFC_DRIFT_SCORE: &str = "RFC Drift Score";
}

pub mod rfc_comments {
    pub const DB_NAME: &str = "ENGRAM/RFC Comments";
    pub const COMMENT: &str = "Comment";
    pub const RFC: &str = "RFC";
    pub const AUTHOR: &str = "Author";
    pub const TYPE: &str = "Type";
    pub const RESOLVED: &str = "Resolved";
    pub const POSTED_AT: &str = "Posted At";
}

// ─── ENGRAM/Pulse ───

pub mod benchmarks {
    pub const DB_NAME: &str = "ENGRAM/Benchmarks";
    pub const BENCHMARK_ID: &str = "Benchmark ID";
    pub const NAME: &str = "Benchmark Name";
    pub const METRIC_TYPE: &str = "Metric Type";
    pub const VALUE: &str = "Value";
    pub const UNIT: &str = "Unit";
    pub const COMMIT_SHA: &str = "Commit SHA";
    pub const BRANCH: &str = "Branch";
    pub const BASELINE_VALUE: &str = "Baseline Value";
    pub const DELTA_PCT: &str = "Delta Pct";
    pub const STATUS: &str = "Status";
    pub const PROJECT: &str = "Project ID";
    pub const RELATED_RFC: &str = "Related RFC";
    pub const RELATED_PR: &str = "Related PR";
    pub const TOOL: &str = "Tool";
    pub const CI_RUN_URL: &str = "CI Run URL";
    pub const TIMESTAMP: &str = "Timestamp";
}

pub mod regressions {
    pub const DB_NAME: &str = "ENGRAM/Regressions";
    pub const REGRESSION_ID: &str = "Regression ID";
    pub const METRIC_NAME: &str = "Metric Name";
    pub const DELTA_PCT: &str = "Delta Pct";
    pub const BASELINE_VALUE: &str = "Baseline Value";
    pub const CURRENT_VALUE: &str = "Current Value";
    pub const SEVERITY: &str = "Severity";
    pub const STATUS: &str = "Status";
    pub const COMMIT_SHA: &str = "Commit SHA";
    pub const PROJECT: &str = "Project ID";
    pub const AFFECTED_BENCHMARK: &str = "Affected Benchmark";
    pub const COMMIT_RANGE: &str = "Commit Range";
    pub const SUSPECTED_CAUSE: &str = "Suspected Cause";
    pub const BISECT_COMMAND: &str = "Bisect Command";
    pub const RESOLUTION_RFC: &str = "Resolution RFC";
    pub const IMPACT_ASSESSMENT: &str = "Impact Assessment";
    pub const ASSIGNED_TO: &str = "Assigned To";
    pub const OPENED_AT: &str = "Opened At";
    pub const RESOLVED_AT: &str = "Resolved At";
    pub const POST_MORTEM: &str = "Post-Mortem";
}

pub mod performance_baselines {
    pub const DB_NAME: &str = "ENGRAM/Performance Baselines";
    pub const BASELINE_ID: &str = "Baseline ID";
    pub const METRIC_NAME: &str = "Metric Name";
    pub const PROJECT: &str = "Project ID";
    pub const ROLLING_MEAN: &str = "Rolling Mean";
    pub const ROLLING_STDDEV: &str = "Rolling Stddev";
    pub const WINDOW_SIZE: &str = "Window Size";
    pub const WARNING_THRESHOLD_PCT: &str = "Warning Threshold %";
    pub const CRITICAL_THRESHOLD_PCT: &str = "Critical Threshold %";
    pub const LAST_UPDATED: &str = "Last Updated";
}

// ─── ENGRAM/Shield ───

pub mod dependencies {
    pub const DB_NAME: &str = "ENGRAM/Dependencies";
    pub const PACKAGE_NAME: &str = "Package";
    pub const PACKAGE_ID: &str = "Package ID";
    pub const VERSION: &str = "Version";
    pub const CVE_ID: &str = "CVE ID";
    pub const SEVERITY: &str = "Severity";
    pub const TRIAGE_STATUS: &str = "Triage Status";
    pub const TITLE: &str = "Title";
    pub const URL: &str = "URL";
    pub const PROJECT: &str = "Project ID";
    pub const COMMIT_SHA: &str = "Commit SHA";
    pub const CVE_IDS: &str = "CVE ID";
    pub const CVSS_SCORE: &str = "CVSS Score";
    pub const FIX_AVAILABLE: &str = "Fix Available";
    pub const FIXED_IN_VERSION: &str = "Fixed In Version";
    pub const AI_RECOMMENDATION: &str = "AI Recommendation";
    pub const TRIAGE_REASONING: &str = "Triage Reasoning";
    pub const RISK_ACCEPTED_BY: &str = "Risk Accepted By";
    pub const FIX_DUE_DATE: &str = "Fix Due Date";
    pub const AFFECTED_PROJECTS: &str = "Affected Projects";
    pub const AFFECTED_MODULES: &str = "Affected Modules";
    pub const RELATED_RFC: &str = "Related RFC";
    pub const FIRST_SEEN: &str = "First Seen";
    pub const LAST_VERIFIED: &str = "Last Verified";
    pub const EXPLOIT_AVAILABLE: &str = "Exploit Available";
    pub const DIRECT_TRANSITIVE: &str = "Direct/Transitive";
}

pub mod audit_runs {
    pub const DB_NAME: &str = "ENGRAM/Audit Runs";
    pub const RUN_NAME: &str = "Run Name";
    pub const TOOL: &str = "Tool";
    pub const FINDINGS_COUNT: &str = "Findings Count";
    pub const CRITICAL_COUNT: &str = "Critical Count";
    pub const HIGH_COUNT: &str = "High Count";
    pub const COMMIT_SHA: &str = "Commit SHA";
    pub const BRANCH: &str = "Branch";
    pub const PROJECT: &str = "Project ID";
}

// ─── ENGRAM/Atlas ───

pub mod modules {
    pub const DB_NAME: &str = "ENGRAM/Modules";
    pub const MODULE_NAME: &str = "Module Name";
    pub const SUMMARY: &str = "Summary";
    pub const COMPLEXITY_SCORE: &str = "Complexity Score";
    pub const LAST_UPDATED: &str = "Last Updated";
    pub const STATUS: &str = "Status";
    pub const PROJECT: &str = "Project ID";
}

pub mod onboarding_tracks {
    pub const DB_NAME: &str = "ENGRAM/Onboarding Tracks";
    pub const TRACK_NAME: &str = "Track Name";
    pub const ENGINEER: &str = "Engineer";
    pub const ROLE: &str = "Role";
    pub const PROJECT: &str = "Project ID";
    pub const PROGRESS: &str = "Progress";
    pub const ESTIMATED_HOURS: &str = "Estimated Hours";
    pub const STEPS: &str = "Steps";
    pub const STEP_COUNT: &str = "Step Count";
    pub const COMPLETION_CRITERIA: &str = "Completion Criteria";
    pub const PREREQUISITES: &str = "Prerequisites";
    pub const LAST_UPDATED: &str = "Last Updated";
    pub const UPDATED_BY: &str = "Updated By";
}

pub mod onboarding_steps {
    pub const DB_NAME: &str = "ENGRAM/Onboarding Steps";
    pub const STEP_TITLE: &str = "Step Name";
    pub const TRACK: &str = "Track ID";
    pub const ORDER: &str = "Order";
    pub const DESCRIPTION: &str = "Description";
    pub const COMPLETED: &str = "Completed";
    pub const WEEK_DAY: &str = "Week/Day";
    pub const TYPE: &str = "Type";
    pub const RELATED_MODULE: &str = "Related Module";
    pub const RELATED_RFC: &str = "Related RFC";
    pub const RELATED_ENV_VARS: &str = "Related Env Vars";
    pub const ESTIMATED_TIME: &str = "Estimated Time";
    pub const VERIFICATION: &str = "Verification";
    pub const AUTO_GENERATED: &str = "Completed";
}

pub mod knowledge_gaps {
    pub const DB_NAME: &str = "ENGRAM/Knowledge Gaps";
    pub const GAP_TITLE: &str = "Gap Title";
    pub const MODULE: &str = "Module";
    pub const SEVERITY: &str = "Severity";
    pub const STATUS: &str = "Status";
    pub const DESCRIPTION: &str = "Description";
    pub const PROJECT: &str = "Project ID";
}

// ─── ENGRAM/Vault ───

pub mod env_config {
    pub const DB_NAME: &str = "ENGRAM/Env Config";
    pub const VAR_NAME: &str = "Var Name";
    pub const ENVIRONMENT: &str = "Environment";
    pub const STATUS: &str = "Status";
    pub const ROTATION_POLICY: &str = "Rotation Policy";
    pub const LAST_ROTATED: &str = "Last Rotated";
    pub const NEXT_ROTATION_DUE: &str = "Next Rotation";
    pub const PROJECT: &str = "Project ID";
    pub const TYPE: &str = "Type";
    pub const DESCRIPTION: &str = "Description";
    pub const MODULE_OWNER: &str = "Module Owner";
    pub const SENSITIVITY: &str = "Sensitivity";
    pub const ROTATION_OWNER: &str = "Rotation Owner";
    pub const INTRODUCED_BY_RFC: &str = "Introduced By RFC";
    pub const PRESENT_IN_DEV: &str = "Present In Dev";
    pub const PRESENT_IN_STAGING: &str = "Present In Staging";
    pub const PRESENT_IN_PROD: &str = "Present In Prod";
    pub const PRESENT_IN_CI: &str = "Present In CI";
    pub const SYNC_STATUS: &str = "Sync Status";
    pub const ONBOARDING_STEP: &str = "Onboarding Step";
}

pub mod config_snapshots {
    pub const DB_NAME: &str = "ENGRAM/Config Snapshots";
    pub const SNAPSHOT_ID: &str = "Snapshot ID";
    pub const PROJECT: &str = "Project ID";
    pub const ENVIRONMENT: &str = "Environment";
    pub const TOTAL_VARS: &str = "Total Vars";
    pub const MISSING_VARS: &str = "Missing Vars";
    pub const OUTDATED_VARS: &str = "Outdated Vars";
    pub const SYNC_ISSUES: &str = "Sync Issues";
    pub const AI_NOTES: &str = "AI Notes";
    pub const SNAPSHOT_AT: &str = "Snapshot At";
}

pub mod secret_rotation_log {
    pub const DB_NAME: &str = "ENGRAM/Secret Rotation Log";
    pub const SECRET_NAME: &str = "Secret Name";
    pub const LOG_ID: &str = "Log ID";
    pub const VAR: &str = "Var";
    pub const ROTATED_BY: &str = "Rotated By";
    pub const OLD_EXPIRY: &str = "Old Expiry";
    pub const NEW_EXPIRY: &str = "New Expiry";
    pub const PROJECT: &str = "Project ID";
    pub const REASON: &str = "Reason";
    pub const ROTATED_AT: &str = "Rotated At";
    pub const NOTES: &str = "Notes";
}

// ─── ENGRAM/Review ───

pub mod pr_reviews {
    pub const DB_NAME: &str = "ENGRAM/PR Reviews";
    pub const PR_ID: &str = "PR Title";
    pub const PR_NUMBER: &str = "PR Number";
    pub const REPO: &str = "Repo";
    pub const TITLE: &str = "PR Title";
    pub const AUTHOR: &str = "Author";
    pub const STATUS: &str = "Status";
    pub const BLOCKERS: &str = "Blockers";
    pub const SUGGESTIONS: &str = "Suggestions";
    pub const NITS: &str = "Nits";
    pub const RFC_REFERENCES: &str = "RFC References";
    pub const AI_SUMMARY: &str = "AI Summary";
    pub const PROJECT: &str = "Project ID";
    pub const BRANCH: &str = "Branch";
    pub const TARGET_BRANCH: &str = "Target Branch";
    pub const IMPLEMENTS_RFC: &str = "RFC References";
    pub const CAUSED_REGRESSION: &str = "Caused Regression";
    pub const INTRODUCED_ENV_VARS: &str = "Introduced Env Vars";
    pub const AFFECTED_MODULES: &str = "Affected Modules";
    pub const BLOCKER_COUNT: &str = "Blockers";
    pub const SUGGESTION_COUNT: &str = "Suggestions";
    pub const NIT_COUNT: &str = "Nits";
    pub const CLAUDE_REVIEW_DRAFT: &str = "AI Summary";
    pub const REVIEW_DRAFT_STATUS: &str = "Review Draft Status";
    pub const REVIEW_QUALITY_SCORE: &str = "Review Quality Score";
    pub const PATTERNS_EXEMPLIFIED: &str = "Patterns Exemplified";
    pub const OPENED_AT: &str = "Opened At";
    pub const MERGED_AT: &str = "Merged At";
    pub const REVIEW_POSTED_AT: &str = "Review Posted At";
}

pub mod review_playbook {
    pub const DB_NAME: &str = "ENGRAM/Review Playbook";
    pub const RULE_NAME: &str = "Rule Name";
    pub const RULE_ID: &str = "Rule ID";
    pub const TITLE: &str = "Rule Name";
    pub const CATEGORY: &str = "Category";
    pub const PATTERN: &str = "Pattern";
    pub const ACTION: &str = "Action";
    pub const ACTIVE: &str = "Active";
    pub const PROJECT: &str = "Project ID";
    pub const DESCRIPTION: &str = "Description";
    pub const RATIONALE: &str = "Rationale";
    pub const GOOD_EXAMPLE: &str = "Good Example";
    pub const BAD_EXAMPLE: &str = "Bad Example";
    pub const SEVERITY: &str = "Severity";
    pub const PATTERN_COUNT: &str = "Pattern Count";
    pub const INTRODUCED_AT: &str = "Introduced At";
    pub const INTRODUCED_BY: &str = "Introduced By";
}

pub mod review_patterns {
    pub const DB_NAME: &str = "ENGRAM/Review Patterns";
    pub const PATTERN_NAME: &str = "Pattern Name";
    pub const CATEGORY: &str = "Category";
    pub const FREQUENCY: &str = "Frequency";
    pub const FIRST_SEEN: &str = "First Seen";
    pub const LAST_SEEN: &str = "Last Seen";
    pub const TREND: &str = "Trend";
    pub const PROJECT: &str = "Project ID";
    pub const SEVERITY: &str = "Severity";
    pub const PLAYBOOK_RULE: &str = "Playbook Rule";
    pub const AFFECTED_MODULES: &str = "Affected Modules";
    pub const EXAMPLE_PRS: &str = "Example PRs";
    pub const TECH_DEBT_ITEM: &str = "Tech Debt Item";
    pub const AI_SUMMARY: &str = "AI Summary";
}

pub mod tech_debt {
    pub const DB_NAME: &str = "ENGRAM/Tech Debt";
    pub const DEBT_ITEM: &str = "Debt Title";
    pub const CATEGORY: &str = "Category";
    pub const PRIORITY: &str = "Priority";
    pub const STATUS: &str = "Status";
    pub const DESCRIPTION: &str = "Description";
    pub const PROJECT: &str = "Project ID";
    pub const SOURCE: &str = "Source";
    pub const SOURCE_PATTERN: &str = "Source Pattern";
    pub const SOURCE_RFC: &str = "Source RFC";
    pub const SEVERITY: &str = "Severity";
    pub const EFFORT_ESTIMATE: &str = "Effort Estimate";
    pub const AFFECTED_MODULES: &str = "Affected Modules";
    pub const PROPOSED_RFC: &str = "Proposed RFC";
    pub const ASSIGNED_TO: &str = "Assigned To";
    pub const IDENTIFIED_AT: &str = "Identified At";
    pub const TARGET_RESOLUTION: &str = "Target Resolution";
}

// ─── Cross-Cutting ───

pub mod health_reports {
    pub const DB_NAME: &str = "ENGRAM/Health Reports";
    pub const REPORT_ID: &str = "Report Title";
    pub const OVERALL_SCORE: &str = "Overall Score";
    pub const DECISIONS_HEALTH: &str = "Decisions Score";
    pub const PULSE_HEALTH: &str = "Pulse Score";
    pub const SHIELD_HEALTH: &str = "Shield Score";
    pub const ATLAS_HEALTH: &str = "Atlas Score";
    pub const VAULT_HEALTH: &str = "Vault Score";
    pub const REVIEW_HEALTH: &str = "Review Score";
    pub const GRADE: &str = "Grade";
    pub const AI_NARRATIVE: &str = "AI Narrative";
    pub const PROJECT: &str = "Project ID";
    pub const REPORT_DATE: &str = "Report Date";
    pub const PERIOD: &str = "Period";
    pub const DELTA_FROM_LAST: &str = "Delta From Last";
    pub const KEY_RISKS: &str = "Key Risks";
    pub const KEY_WINS: &str = "Key Wins";
    pub const GENERATED_AT: &str = "Report Date";
}

pub mod engineering_digest {
    pub const DB_NAME: &str = "ENGRAM/Engineering Digest";
    pub const DIGEST_ID: &str = "Digest Title";
    pub const WEEK_OF: &str = "Week Of";
    pub const OVERALL_SCORE: &str = "Overall Score";
    pub const DELTA: &str = "Delta";
    pub const SUMMARY: &str = "Summary";
    pub const KEY_EVENTS: &str = "Key Events";
    pub const RECOMMENDATIONS: &str = "Recommendations";
    pub const SENT_TO_SLACK: &str = "Sent To Slack";
    pub const PROJECT: &str = "Project ID";
    pub const NEW_RFCS: &str = "New RFCs";
    pub const RFCS_APPROVED: &str = "RFCs Approved";
    pub const REGRESSIONS_FOUND: &str = "Regressions Found";
    pub const REGRESSIONS_RESOLVED: &str = "Regressions Resolved";
    pub const NEW_VULNERABILITIES: &str = "New Vulnerabilities";
    pub const VULNERABILITIES_TRIAGED: &str = "Vulnerabilities Triaged";
    pub const PRS_REVIEWED: &str = "PRs Reviewed";
    pub const NEW_PATTERNS: &str = "New Patterns";
    pub const HEALTH_SCORE: &str = "Overall Score";
    pub const HEALTH_DELTA: &str = "Delta";
    pub const NARRATIVE: &str = "Summary";
    pub const NOTABLE_EVENTS: &str = "Key Events";
    pub const ACTION_ITEMS: &str = "Recommendations";
    pub const GENERATED_AT: &str = "Week Of";
}

pub mod events {
    pub const DB_NAME: &str = "ENGRAM/Events";
    pub const TITLE: &str = "Event Title";
    pub const SOURCE_LAYER: &str = "Source Layer";
    pub const TYPE: &str = "Event Type";
    pub const DETAILS: &str = "Details";
    pub const PROJECT: &str = "Project ID";
    pub const TIMESTAMP: &str = "Timestamp";
    pub const IS_MILESTONE: &str = "Is Milestone";
}

pub mod releases {
    pub const DB_NAME: &str = "ENGRAM/Releases";
    pub const RELEASE_ID: &str = "Release Name";
    pub const VERSION: &str = "Version";
    pub const PROJECT: &str = "Project ID";
    pub const STATUS: &str = "Status";
    pub const MILESTONE: &str = "Milestone";
    pub const INCLUDED_PRS: &str = "PRs Included";
    pub const IMPLEMENTED_RFCS: &str = "RFCs Implemented";
    pub const NEW_ENV_VARS: &str = "Env Vars Changed";
    pub const DEPENDENCY_CHANGES: &str = "Dependency Changes";
    pub const ALL_TESTS_PASS: &str = "All Tests Pass";
    pub const REGRESSION_FREE: &str = "Regression Free";
    pub const CVE_FREE: &str = "No Critical CVEs";
    pub const RELEASE_NOTES: &str = "Release Notes";
    pub const MIGRATION_NOTES: &str = "Migration Notes";
    pub const READINESS: &str = "Readiness";
    pub const RELEASED_AT: &str = "Released At";
}
