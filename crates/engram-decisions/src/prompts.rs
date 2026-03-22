/// Claude prompt templates for the Decisions agent.

/// Generates a prompt that asks Claude to compare a PR diff against an RFC proposal
/// and produce a Decision Rationale with an RFC Drift Score.
///
/// The expected JSON response schema:
/// ```json
/// {
///   "decision_rationale": "...",
///   "drift_score": 0-10,
///   "drift_notes": "..."
/// }
/// ```
pub fn decision_rationale_prompt(rfc_title: &str, rfc_body: &str, pr_diff: &str) -> String {
    format!(
        r#"You are an architecture-governance assistant for the ENGRAM system.

Compare the following PR diff against the original RFC proposal and produce a Decision Rationale.

## RFC: {rfc_title}
{rfc_body}

## PR Diff
```
{pr_diff}
```

Respond with a JSON object containing exactly these fields:
- "decision_rationale": A concise summary of how the implementation aligns with or diverges from the RFC proposal.
- "drift_score": An integer 0-10 where 0 = perfect alignment and 10 = complete divergence.
- "drift_notes": Bullet-point notes explaining each area of drift, or "No drift detected" if score is 0.

Respond ONLY with the JSON object, no surrounding text."#
    )
}

/// Generates a prompt for auto-creating an RFC draft when a regression is detected.
pub fn regression_rfc_draft_prompt(metric_name: &str, delta_pct: f64, related_pr: Option<&str>) -> String {
    let pr_context = match related_pr {
        Some(pr) => format!("Related PR: {pr}"),
        None => "No related PR identified.".to_string(),
    };
    format!(
        r#"You are an architecture-governance assistant for the ENGRAM system.

A performance regression has been detected:
- Metric: {metric_name}
- Delta: {delta_pct:.1}%
- {pr_context}

Draft a short RFC proposal body (3-5 paragraphs) to investigate this regression.
The RFC title will be: "Investigate {metric_name} regression"

Include:
1. Problem Statement: Describe what regressed and by how much.
2. Impact Assessment: Potential user/system impact.
3. Investigation Plan: Steps to root-cause the regression.
4. Proposed Mitigation: Initial ideas to fix or mitigate.

Respond with plain text (no JSON wrapping)."#
    )
}

/// Generates a prompt for auto-creating an RFC draft when a CVE is detected.
pub fn cve_rfc_draft_prompt(package_name: &str, cve_ids: &[String], severity: &str) -> String {
    let cve_list = cve_ids.join(", ");
    format!(
        r#"You are an architecture-governance assistant for the ENGRAM system.

A security vulnerability has been detected:
- Package: {package_name}
- CVEs: {cve_list}
- Severity: {severity}

Draft a short RFC proposal body (3-5 paragraphs) to remediate this vulnerability.
The RFC title will be: "Remediate {cve_list} in {package_name}"

Include:
1. Vulnerability Summary: Describe the CVEs and their severity.
2. Impact Assessment: What parts of the system are affected.
3. Remediation Plan: Steps to patch or upgrade the affected dependency.
4. Verification: How to confirm the fix resolves the vulnerability.

Respond with plain text (no JSON wrapping)."#
    )
}
