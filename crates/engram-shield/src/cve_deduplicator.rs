//! CVE deduplication logic for the Shield agent.
//!
//! Uses `Package ID` (`{ecosystem}/{package}@{version}`) as the idempotency
//! key against the Notion Dependencies database.

use std::collections::HashSet;

use crate::audit_parser::VulnFinding;

/// Build a canonical Package ID used as the idempotency key in the
/// Dependencies database.
///
/// Format: `{ecosystem}/{package_name}@{version}`
pub fn generate_package_id(ecosystem: &str, package_name: &str, version: &str) -> String {
    format!("{ecosystem}/{package_name}@{version}")
}

/// Extract the Package ID from a [`VulnFinding`].
pub fn package_id_for(finding: &VulnFinding) -> String {
    generate_package_id(&finding.ecosystem, &finding.package_name, &finding.version)
}

/// Result of deduplication: new findings to insert and existing findings
/// whose `Last Verified` timestamp should be bumped.
#[derive(Debug)]
pub struct DeduplicationResult {
    /// Findings that do not yet exist in the Dependencies DB.
    pub new_findings: Vec<VulnFinding>,
    /// Findings that already exist (by Package ID) and need a timestamp update.
    pub existing_findings: Vec<VulnFinding>,
}

/// Deduplicate a batch of [`VulnFinding`]s against a set of Package IDs
/// already present in the Notion Dependencies database.
///
/// Returns a [`DeduplicationResult`] splitting findings into new vs existing.
pub fn deduplicate(
    findings: Vec<VulnFinding>,
    existing_ids: &HashSet<String>,
) -> DeduplicationResult {
    let mut new_findings = Vec::new();
    let mut existing_findings = Vec::new();

    // Track IDs we have already classified in this batch to avoid duplicates
    // within a single audit run.
    let mut seen_in_batch: HashSet<String> = HashSet::new();

    for finding in findings {
        let pid = package_id_for(&finding);

        // Skip intra-batch duplicates
        if !seen_in_batch.insert(pid.clone()) {
            continue;
        }

        if existing_ids.contains(&pid) {
            existing_findings.push(finding);
        } else {
            new_findings.push(finding);
        }
    }

    DeduplicationResult {
        new_findings,
        existing_findings,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit_parser::VulnFinding;
    use engram_types::events::Severity;

    fn make_finding(name: &str, version: &str) -> VulnFinding {
        VulnFinding {
            package_name: name.to_string(),
            version: version.to_string(),
            ecosystem: "crates.io".to_string(),
            cve_ids: vec!["CVE-2024-0001".to_string()],
            cvss_score: Some(7.5),
            severity: Severity::High,
            fix_available: true,
            fixed_version: Some("2.0.0".to_string()),
            description: "Test".to_string(),
        }
    }

    #[test]
    fn test_generate_package_id() {
        assert_eq!(
            generate_package_id("crates.io", "serde", "1.0.0"),
            "crates.io/serde@1.0.0"
        );
    }

    #[test]
    fn test_deduplicate_splits_correctly() {
        let findings = vec![
            make_finding("new-crate", "1.0.0"),
            make_finding("existing-crate", "2.0.0"),
            make_finding("new-crate", "1.0.0"), // intra-batch duplicate
        ];

        let mut existing = HashSet::new();
        existing.insert("crates.io/existing-crate@2.0.0".to_string());

        let result = deduplicate(findings, &existing);
        assert_eq!(result.new_findings.len(), 1);
        assert_eq!(result.new_findings[0].package_name, "new-crate");
        assert_eq!(result.existing_findings.len(), 1);
        assert_eq!(result.existing_findings[0].package_name, "existing-crate");
    }
}
