//! Parsers for security audit tool JSON output.
//!
//! Each function accepts the raw JSON string produced by the corresponding
//! audit tool and returns a normalised `Vec<VulnFinding>`.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use engram_types::events::{AuditTool, Severity};

// ────────────────────────────────────────────────────────────────────────────
// Normalised vulnerability finding
// ────────────────────────────────────────────────────────────────────────────

/// A single vulnerability finding normalised across all audit tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnFinding {
    pub package_name: String,
    pub version: String,
    pub ecosystem: String,
    pub cve_ids: Vec<String>,
    pub cvss_score: Option<f64>,
    pub severity: Severity,
    pub fix_available: bool,
    pub fixed_version: Option<String>,
    pub description: String,
}

/// Dispatch to the correct parser based on [`AuditTool`].
pub fn parse_audit_output(tool: &AuditTool, raw: &str) -> Result<Vec<VulnFinding>> {
    match tool {
        AuditTool::CargoAudit => parse_cargo_audit(raw),
        AuditTool::NpmAudit => parse_npm_audit(raw),
        AuditTool::PipAudit => parse_pip_audit(raw),
        AuditTool::OsvScanner => parse_osv_scanner(raw),
    }
}

// ────────────────────────────────────────────────────────────────────────────
// cargo-audit  (cargo audit --json)
// ────────────────────────────────────────────────────────────────────────────

/// Intermediate serde model for `cargo audit --json` output.
#[derive(Deserialize)]
struct CargoAuditReport {
    #[serde(default)]
    vulnerabilities: CargoVulnerabilities,
}

#[derive(Deserialize, Default)]
struct CargoVulnerabilities {
    #[serde(default)]
    list: Vec<CargoVuln>,
}

#[derive(Deserialize)]
struct CargoVuln {
    advisory: CargoAdvisory,
    #[serde(default)]
    versions: Option<CargoVersions>,
    package: CargoPackage,
}

#[derive(Deserialize)]
struct CargoAdvisory {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    cvss: Option<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

#[derive(Deserialize)]
struct CargoVersions {
    #[serde(default)]
    patched: Vec<String>,
}

#[derive(Deserialize)]
struct CargoPackage {
    name: String,
    version: String,
}

pub fn parse_cargo_audit(raw: &str) -> Result<Vec<VulnFinding>> {
    let report: CargoAuditReport =
        serde_json::from_str(raw).context("Failed to parse cargo-audit JSON")?;

    let findings = report
        .vulnerabilities
        .list
        .into_iter()
        .map(|v| {
            // Collect CVE-like IDs: the advisory id itself plus any aliases
            let mut cve_ids: Vec<String> = v
                .advisory
                .aliases
                .iter()
                .filter(|a| a.starts_with("CVE-"))
                .cloned()
                .collect();
            if v.advisory.id.starts_with("CVE-") {
                cve_ids.insert(0, v.advisory.id.clone());
            }
            // If no CVE alias, keep the RUSTSEC id
            if cve_ids.is_empty() {
                cve_ids.push(v.advisory.id.clone());
            }

            let cvss_score = v
                .advisory
                .cvss
                .as_deref()
                .and_then(|s| parse_cvss_score(Some(s)));
            let severity = cvss_to_severity(cvss_score);

            let patched = v
                .versions
                .as_ref()
                .map(|vs| vs.patched.clone())
                .unwrap_or_default();
            let fix_available = !patched.is_empty();
            let fixed_version = patched.first().cloned();

            VulnFinding {
                package_name: v.package.name,
                version: v.package.version,
                ecosystem: "crates.io".to_string(),
                cve_ids,
                cvss_score,
                severity,
                fix_available,
                fixed_version,
                description: v.advisory.title,
            }
        })
        .collect();

    Ok(findings)
}

// ────────────────────────────────────────────────────────────────────────────
// npm audit  (npm audit --json)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct NpmAuditReport {
    #[serde(default)]
    vulnerabilities: serde_json::Map<String, serde_json::Value>,
}

pub fn parse_npm_audit(raw: &str) -> Result<Vec<VulnFinding>> {
    let report: NpmAuditReport =
        serde_json::from_str(raw).context("Failed to parse npm-audit JSON")?;

    let mut findings = Vec::new();

    for (pkg_name, val) in &report.vulnerabilities {
        let severity_str = val
            .get("severity")
            .and_then(|s| s.as_str())
            .unwrap_or("info");
        let severity = parse_severity_string(severity_str);

        // npm v7+ "via" can contain advisory objects or package name strings
        let mut cve_ids = Vec::new();
        let mut description = String::new();
        let mut cvss_score: Option<f64> = None;

        if let Some(via) = val.get("via").and_then(|v| v.as_array()) {
            for item in via {
                if let Some(obj) = item.as_object() {
                    if let Some(cve) = obj.get("cve").and_then(|c| c.as_str()) {
                        if !cve.is_empty() {
                            cve_ids.push(cve.to_string());
                        }
                    }
                    if description.is_empty() {
                        if let Some(t) = obj.get("title").and_then(|t| t.as_str()) {
                            description = t.to_string();
                        }
                    }
                    if cvss_score.is_none() {
                        cvss_score = obj.get("cvss").and_then(|c| {
                            c.get("score").and_then(|s| s.as_f64())
                        });
                    }
                }
            }
        }

        let range = val
            .get("range")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string();

        let fix_available = val
            .get("fixAvailable")
            .map(|f| f.as_bool().unwrap_or(false) || f.is_object())
            .unwrap_or(false);

        // Extract version from the "nodes" array or "range"
        let version = val
            .get("nodes")
            .and_then(|n| n.as_array())
            .and_then(|arr| arr.first())
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();

        if cve_ids.is_empty() {
            cve_ids.push(format!("npm-vuln-{pkg_name}"));
        }

        if description.is_empty() {
            description = format!("Vulnerability in {pkg_name} ({range})");
        }

        findings.push(VulnFinding {
            package_name: pkg_name.clone(),
            version,
            ecosystem: "npm".to_string(),
            cve_ids,
            cvss_score,
            severity,
            fix_available,
            fixed_version: None,
            description,
        });
    }

    Ok(findings)
}

// ────────────────────────────────────────────────────────────────────────────
// pip-audit  (pip-audit --format=json)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PipAuditReport {
    #[serde(default)]
    dependencies: Vec<PipDependency>,
}

#[derive(Deserialize)]
struct PipDependency {
    name: String,
    version: String,
    #[serde(default)]
    vulns: Vec<PipVuln>,
}

#[derive(Deserialize)]
struct PipVuln {
    id: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    fix_versions: Vec<String>,
    #[serde(default)]
    aliases: Vec<String>,
}

pub fn parse_pip_audit(raw: &str) -> Result<Vec<VulnFinding>> {
    // pip-audit can output either { "dependencies": [...] } or a top-level array
    let deps: Vec<PipDependency> = if raw.trim_start().starts_with('[') {
        serde_json::from_str(raw).context("Failed to parse pip-audit JSON array")?
    } else {
        let report: PipAuditReport =
            serde_json::from_str(raw).context("Failed to parse pip-audit JSON")?;
        report.dependencies
    };

    let mut findings = Vec::new();

    for dep in deps {
        for vuln in &dep.vulns {
            let mut cve_ids: Vec<String> = vuln
                .aliases
                .iter()
                .filter(|a| a.starts_with("CVE-"))
                .cloned()
                .collect();
            if vuln.id.starts_with("CVE-") || vuln.id.starts_with("PYSEC-") || vuln.id.starts_with("GHSA-") {
                cve_ids.insert(0, vuln.id.clone());
            }
            if cve_ids.is_empty() {
                cve_ids.push(vuln.id.clone());
            }

            let fix_available = !vuln.fix_versions.is_empty();
            let fixed_version = vuln.fix_versions.first().cloned();

            findings.push(VulnFinding {
                package_name: dep.name.clone(),
                version: dep.version.clone(),
                ecosystem: "pypi".to_string(),
                cve_ids,
                cvss_score: None,
                severity: Severity::Medium, // pip-audit doesn't provide severity inline
                fix_available,
                fixed_version,
                description: vuln.description.clone(),
            });
        }
    }

    Ok(findings)
}

// ────────────────────────────────────────────────────────────────────────────
// osv-scanner (osv-scanner --json)
// ────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OsvReport {
    #[serde(default)]
    results: Vec<OsvResult>,
}

#[derive(Deserialize)]
struct OsvResult {
    #[serde(default)]
    packages: Vec<OsvPackageResult>,
}

#[derive(Deserialize)]
struct OsvPackageResult {
    #[serde(default, rename = "package")]
    pkg: OsvPackage,
    #[serde(default)]
    vulnerabilities: Vec<OsvVuln>,
}

#[derive(Deserialize, Default)]
struct OsvPackage {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    ecosystem: String,
}

#[derive(Deserialize)]
struct OsvVuln {
    id: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    aliases: Vec<String>,
    #[serde(default)]
    severity: Vec<OsvSeverity>,
    #[serde(default)]
    affected: Vec<OsvAffected>,
}

#[derive(Deserialize)]
struct OsvSeverity {
    #[serde(default, rename = "type")]
    score_type: String,
    #[serde(default)]
    score: String,
}

#[derive(Deserialize)]
struct OsvAffected {
    #[serde(default)]
    ranges: Vec<OsvRange>,
}

#[derive(Deserialize)]
struct OsvRange {
    #[serde(default)]
    events: Vec<OsvRangeEvent>,
}

#[derive(Deserialize)]
struct OsvRangeEvent {
    #[serde(default)]
    fixed: Option<String>,
}

pub fn parse_osv_scanner(raw: &str) -> Result<Vec<VulnFinding>> {
    let report: OsvReport =
        serde_json::from_str(raw).context("Failed to parse osv-scanner JSON")?;

    let mut findings = Vec::new();

    for result in &report.results {
        for pkg_result in &result.packages {
            let pkg = &pkg_result.pkg;
            let ecosystem = normalise_ecosystem(&pkg.ecosystem);

            for vuln in &pkg_result.vulnerabilities {
                let mut cve_ids: Vec<String> = vuln
                    .aliases
                    .iter()
                    .filter(|a| a.starts_with("CVE-"))
                    .cloned()
                    .collect();
                if vuln.id.starts_with("CVE-") {
                    cve_ids.insert(0, vuln.id.clone());
                }
                if cve_ids.is_empty() {
                    cve_ids.push(vuln.id.clone());
                }

                // Extract CVSS score from severity entries
                let cvss_score = vuln
                    .severity
                    .iter()
                    .find(|s| s.score_type == "CVSS_V3")
                    .and_then(|s| parse_cvss_score(Some(&s.score)));
                let severity = cvss_to_severity(cvss_score);

                // Find fixed version from affected ranges
                let fixed_version = vuln
                    .affected
                    .iter()
                    .flat_map(|a| &a.ranges)
                    .flat_map(|r| &r.events)
                    .find_map(|e| e.fixed.clone());
                let fix_available = fixed_version.is_some();

                findings.push(VulnFinding {
                    package_name: pkg.name.clone(),
                    version: pkg.version.clone(),
                    ecosystem: ecosystem.clone(),
                    cve_ids,
                    cvss_score,
                    severity,
                    fix_available,
                    fixed_version,
                    description: vuln.summary.clone(),
                });
            }
        }
    }

    Ok(findings)
}

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

/// Parse a CVSS vector string (e.g. "CVSS:3.1/AV:N/AC:L/..." ) or a bare score
/// into an `f64` base score.  Returns `None` on parse failure.
fn parse_cvss_score(raw: Option<&str>) -> Option<f64> {
    let raw = raw?;
    // If it is just a float, parse directly
    if let Ok(score) = raw.parse::<f64>() {
        return Some(score);
    }
    // Otherwise ignore the vector — we would need a full CVSS calculator.
    // Some tools embed the score after the vector string.
    None
}

/// Map a numeric CVSS score to a [`Severity`].
fn cvss_to_severity(score: Option<f64>) -> Severity {
    match score {
        Some(s) if s >= 9.0 => Severity::Critical,
        Some(s) if s >= 7.0 => Severity::High,
        Some(s) if s >= 4.0 => Severity::Medium,
        Some(s) if s > 0.0 => Severity::Low,
        _ => Severity::Medium, // default when unknown
    }
}

/// Map an npm/pip severity keyword to [`Severity`].
fn parse_severity_string(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "critical" => Severity::Critical,
        "high" => Severity::High,
        "moderate" | "medium" => Severity::Medium,
        "low" => Severity::Low,
        _ => Severity::Info,
    }
}

/// Normalise OSV ecosystem names to our canonical form.
fn normalise_ecosystem(eco: &str) -> String {
    match eco {
        "crates.io" | "Cargo" => "crates.io".to_string(),
        "npm" | "Node" => "npm".to_string(),
        "PyPI" | "pip" => "pypi".to_string(),
        "Go" => "go".to_string(),
        other => other.to_lowercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cvss_to_severity() {
        assert_eq!(cvss_to_severity(Some(9.8)), Severity::Critical);
        assert_eq!(cvss_to_severity(Some(7.5)), Severity::High);
        assert_eq!(cvss_to_severity(Some(5.0)), Severity::Medium);
        assert_eq!(cvss_to_severity(Some(2.0)), Severity::Low);
        assert_eq!(cvss_to_severity(None), Severity::Medium);
    }

    #[test]
    fn test_parse_severity_string() {
        assert_eq!(parse_severity_string("critical"), Severity::Critical);
        assert_eq!(parse_severity_string("High"), Severity::High);
        assert_eq!(parse_severity_string("moderate"), Severity::Medium);
        assert_eq!(parse_severity_string("low"), Severity::Low);
        assert_eq!(parse_severity_string("unknown"), Severity::Info);
    }

    #[test]
    fn test_parse_cargo_audit_minimal() {
        let json = r#"{
            "vulnerabilities": {
                "list": [
                    {
                        "advisory": {
                            "id": "RUSTSEC-2023-0001",
                            "title": "Test vuln",
                            "aliases": ["CVE-2023-9999"]
                        },
                        "versions": { "patched": ["1.2.3"] },
                        "package": { "name": "some-crate", "version": "1.0.0" }
                    }
                ]
            }
        }"#;
        let findings = parse_cargo_audit(json).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package_name, "some-crate");
        assert_eq!(findings[0].cve_ids, vec!["CVE-2023-9999"]);
        assert!(findings[0].fix_available);
        assert_eq!(findings[0].fixed_version.as_deref(), Some("1.2.3"));
    }

    #[test]
    fn test_parse_osv_scanner_minimal() {
        let json = r#"{
            "results": [
                {
                    "packages": [
                        {
                            "package": {
                                "name": "lodash",
                                "version": "4.17.20",
                                "ecosystem": "npm"
                            },
                            "vulnerabilities": [
                                {
                                    "id": "GHSA-xxxx-yyyy-zzzz",
                                    "summary": "Prototype pollution",
                                    "aliases": ["CVE-2021-23337"],
                                    "severity": [],
                                    "affected": []
                                }
                            ]
                        }
                    ]
                }
            ]
        }"#;
        let findings = parse_osv_scanner(json).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].package_name, "lodash");
        assert_eq!(findings[0].cve_ids[0], "CVE-2021-23337");
    }
}
