//! Claude prompt templates for the Shield agent.
//!
//! All prompts are pure functions that return a `String`. The caller is
//! responsible for sending the prompt to the Claude API via the MCP bridge.

use crate::audit_parser::VulnFinding;

/// Build a CVE triage prompt that asks Claude to classify a vulnerability
/// and recommend a course of action.
///
/// The response schema is structured so the caller can parse:
///   - `DEPENDENCY_TYPE`: direct | transitive
///   - `EXPLOITABILITY`: remote-no-auth | remote-auth | local | theoretical
///   - `TRIAGE_DECISION`: fix-immediately | schedule-fix | accept-risk | needs-investigation
///   - `FIX_APPROACH`: free-form remediation guidance
pub fn cve_triage_prompt(finding: &VulnFinding, project_context: &str) -> String {
    let cve_list = finding.cve_ids.join(", ");
    let cvss = finding
        .cvss_score
        .map(|s| format!("{s:.1}"))
        .unwrap_or_else(|| "unknown".to_string());
    let fix_info = if finding.fix_available {
        format!(
            "A fix is available in version {}.",
            finding
                .fixed_version
                .as_deref()
                .unwrap_or("(unspecified)")
        )
    } else {
        "No upstream fix is currently available.".to_string()
    };

    format!(
        r#"You are a security engineer performing CVE triage for the ENGRAM project management system.

## Vulnerability Details
- **Package**: {pkg} @ {version}
- **Ecosystem**: {ecosystem}
- **CVE IDs**: {cve_list}
- **CVSS Score**: {cvss}
- **Severity**: {severity}
- **Description**: {description}
- **Fix Status**: {fix_info}

## Project Context
{project_context}

## Instructions
Analyse the vulnerability above and provide a structured triage assessment.
Consider the package ecosystem, whether the package is likely a direct or
transitive dependency, the exploitability given the project's deployment model,
and the best remediation path.

Respond with EXACTLY the following fields (one per line, colon-separated):

DEPENDENCY_TYPE: <direct | transitive>
EXPLOITABILITY: <remote-no-auth | remote-auth | local | theoretical>
TRIAGE_DECISION: <fix-immediately | schedule-fix | accept-risk | needs-investigation>
FIX_APPROACH: <concise remediation guidance, 1-3 sentences>
REASONING: <brief explanation of your assessment, 2-4 sentences>"#,
        pkg = finding.package_name,
        version = finding.version,
        ecosystem = finding.ecosystem,
        severity = finding.severity,
        description = finding.description,
    )
}

/// Build a prompt asking Claude to draft an RFC body for a critical/high CVE
/// that needs immediate remediation.
pub fn cve_rfc_draft_prompt(finding: &VulnFinding, ai_recommendation: &str) -> String {
    let cve_list = finding.cve_ids.join(", ");
    let cvss = finding
        .cvss_score
        .map(|s| format!("{s:.1}"))
        .unwrap_or_else(|| "unknown".to_string());

    format!(
        r#"You are a senior engineer drafting an RFC for an urgent security remediation.

## Vulnerability
- **Package**: {pkg} @ {version}
- **CVE IDs**: {cve_list}
- **CVSS Score**: {cvss}
- **Severity**: {severity}
- **AI Triage Recommendation**: {ai_recommendation}

## Task
Draft a concise RFC with the following sections:

### Problem Statement
Describe the security risk this CVE poses to the project.

### Proposed Solution
Outline the remediation approach (upgrade, patch, replace, or workaround).

### Alternatives Considered
List at least two alternative approaches with trade-offs.

### Trade-offs
Describe any breaking changes, performance implications, or effort required.

### Decision Rationale
Explain why the proposed solution is the best path forward.

Return the RFC as plain Markdown."#,
        pkg = finding.package_name,
        version = finding.version,
        severity = finding.severity,
    )
}

/// Build a prompt to summarise the overall audit run for a project.
pub fn audit_summary_prompt(
    total: usize,
    critical: usize,
    high: usize,
    new_count: usize,
    resolved_count: usize,
) -> String {
    format!(
        r#"You are a security analyst summarising a dependency audit run for the ENGRAM system.

## Audit Results
- **Total findings**: {total}
- **Critical**: {critical}
- **High**: {high}
- **New findings this run**: {new_count}
- **Resolved since last run**: {resolved_count}

Write a brief 2-3 sentence summary of the security posture based on these numbers.
Highlight the most urgent items and whether the trend is improving or worsening."#
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_parser::VulnFinding;
    use engram_types::events::Severity;

    fn sample_finding() -> VulnFinding {
        VulnFinding {
            package_name: "openssl".to_string(),
            version: "1.1.1".to_string(),
            ecosystem: "crates.io".to_string(),
            cve_ids: vec!["CVE-2024-1234".to_string()],
            cvss_score: Some(9.8),
            severity: Severity::Critical,
            fix_available: true,
            fixed_version: Some("1.1.2".to_string()),
            description: "Buffer overflow in TLS handshake".to_string(),
        }
    }

    #[test]
    fn triage_prompt_contains_key_fields() {
        let prompt = cve_triage_prompt(&sample_finding(), "A Rust web service");
        assert!(prompt.contains("CVE-2024-1234"));
        assert!(prompt.contains("openssl"));
        assert!(prompt.contains("9.8"));
        assert!(prompt.contains("DEPENDENCY_TYPE"));
        assert!(prompt.contains("EXPLOITABILITY"));
        assert!(prompt.contains("TRIAGE_DECISION"));
        assert!(prompt.contains("FIX_APPROACH"));
    }

    #[test]
    fn rfc_draft_prompt_contains_cve() {
        let prompt = cve_rfc_draft_prompt(&sample_finding(), "Fix immediately");
        assert!(prompt.contains("CVE-2024-1234"));
        assert!(prompt.contains("Problem Statement"));
    }
}
