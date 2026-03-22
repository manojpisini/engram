//! Knowledge gap detection: undocumented modules, stale docs, orphaned RFCs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::info;

/// A detected gap in the project's knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGap {
    /// Human-readable title describing the gap.
    pub title: String,
    /// Classification of the gap.
    pub gap_type: GapType,
    /// How urgent it is to address.
    pub severity: GapSeverity,
    /// The module this gap relates to, if any.
    pub related_module: Option<String>,
    /// The RFC this gap relates to, if any.
    pub related_rfc: Option<String>,
}

/// Classification of a knowledge gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapType {
    /// Module exists in codebase but has no documentation page.
    UndocumentedModule,
    /// Module doc exists but has not been updated recently.
    StaleDocumentation,
    /// RFC has no related module (orphaned design document).
    OrphanedRfc,
}

impl std::fmt::Display for GapType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GapType::UndocumentedModule => write!(f, "Undocumented Module"),
            GapType::StaleDocumentation => write!(f, "Stale Documentation"),
            GapType::OrphanedRfc => write!(f, "Orphaned RFC"),
        }
    }
}

/// Severity of a knowledge gap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GapSeverity {
    High,
    Medium,
    Low,
}

impl std::fmt::Display for GapSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GapSeverity::High => write!(f, "High"),
            GapSeverity::Medium => write!(f, "Medium"),
            GapSeverity::Low => write!(f, "Low"),
        }
    }
}

/// Minimal info about a module as fetched from the Modules database.
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub what_it_does: Option<String>,
    pub last_updated: Option<DateTime<Utc>>,
    pub related_rfcs: Vec<String>,
}

/// Minimal info about an RFC as fetched from the RFCs database.
#[derive(Debug, Clone)]
pub struct RfcInfo {
    pub rfc_id: String,
    pub title: String,
    pub affected_modules: Vec<String>,
}

/// Detect modules that have no documentation (empty or missing `what_it_does`).
pub fn detect_undocumented_modules(modules: &[ModuleInfo]) -> Vec<KnowledgeGap> {
    info!(
        "[GapDetector] Scanning {} modules for missing documentation",
        modules.len()
    );

    modules
        .iter()
        .filter(|m| {
            m.what_it_does
                .as_ref()
                .map_or(true, |s| s.trim().is_empty())
        })
        .map(|m| KnowledgeGap {
            title: format!("Module '{}' has no documentation", m.name),
            gap_type: GapType::UndocumentedModule,
            severity: GapSeverity::High,
            related_module: Some(m.name.clone()),
            related_rfc: None,
        })
        .collect()
}

/// Detect modules whose documentation has not been updated within `threshold_days`.
pub fn detect_stale_docs(modules: &[ModuleInfo], threshold_days: u32) -> Vec<KnowledgeGap> {
    let now = Utc::now();
    let threshold = chrono::Duration::days(i64::from(threshold_days));

    info!(
        "[GapDetector] Scanning {} modules for stale docs (threshold={}d)",
        modules.len(),
        threshold_days
    );

    modules
        .iter()
        .filter(|m| {
            // Only consider modules that have some docs (undocumented ones are caught separately)
            let has_docs = m
                .what_it_does
                .as_ref()
                .map_or(false, |s| !s.trim().is_empty());
            if !has_docs {
                return false;
            }
            match m.last_updated {
                Some(updated) => now.signed_duration_since(updated) > threshold,
                None => true, // No update timestamp means effectively stale
            }
        })
        .map(|m| {
            let days_stale = m
                .last_updated
                .map(|u| now.signed_duration_since(u).num_days())
                .unwrap_or(-1);
            let severity = if days_stale > 180 {
                GapSeverity::High
            } else if days_stale > 90 {
                GapSeverity::Medium
            } else {
                GapSeverity::Low
            };
            KnowledgeGap {
                title: format!(
                    "Module '{}' docs last updated {}d ago",
                    m.name,
                    days_stale.max(0)
                ),
                gap_type: GapType::StaleDocumentation,
                severity,
                related_module: Some(m.name.clone()),
                related_rfc: None,
            }
        })
        .collect()
}

/// Detect RFCs that reference no modules (orphaned design documents).
pub fn detect_orphaned_rfcs(rfcs: &[RfcInfo], modules: &[ModuleInfo]) -> Vec<KnowledgeGap> {
    info!(
        "[GapDetector] Scanning {} RFCs for orphans against {} modules",
        rfcs.len(),
        modules.len()
    );

    let module_names: std::collections::HashSet<&str> =
        modules.iter().map(|m| m.name.as_str()).collect();

    rfcs.iter()
        .filter(|rfc| {
            // Orphaned if no affected modules, or none of its affected modules actually exist
            if rfc.affected_modules.is_empty() {
                return true;
            }
            !rfc.affected_modules
                .iter()
                .any(|am| module_names.contains(am.as_str()))
        })
        .map(|rfc| KnowledgeGap {
            title: format!(
                "RFC '{}' ({}) has no related module",
                rfc.title, rfc.rfc_id
            ),
            gap_type: GapType::OrphanedRfc,
            severity: GapSeverity::Medium,
            related_module: None,
            related_rfc: Some(rfc.rfc_id.clone()),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_detect_undocumented_modules() {
        let modules = vec![
            ModuleInfo {
                name: "auth".into(),
                what_it_does: Some("Handles auth.".into()),
                last_updated: None,
                related_rfcs: vec![],
            },
            ModuleInfo {
                name: "payments".into(),
                what_it_does: None,
                last_updated: None,
                related_rfcs: vec![],
            },
            ModuleInfo {
                name: "logging".into(),
                what_it_does: Some("".into()),
                last_updated: None,
                related_rfcs: vec![],
            },
        ];
        let gaps = detect_undocumented_modules(&modules);
        assert_eq!(gaps.len(), 2);
        assert!(gaps.iter().any(|g| g.related_module.as_deref() == Some("payments")));
        assert!(gaps.iter().any(|g| g.related_module.as_deref() == Some("logging")));
    }

    #[test]
    fn test_detect_stale_docs() {
        let old_date = Utc.with_ymd_and_hms(2023, 1, 1, 0, 0, 0).unwrap();
        let recent_date = Utc::now() - chrono::Duration::days(10);
        let modules = vec![
            ModuleInfo {
                name: "old-mod".into(),
                what_it_does: Some("Does stuff.".into()),
                last_updated: Some(old_date),
                related_rfcs: vec![],
            },
            ModuleInfo {
                name: "new-mod".into(),
                what_it_does: Some("Does other stuff.".into()),
                last_updated: Some(recent_date),
                related_rfcs: vec![],
            },
        ];
        let gaps = detect_stale_docs(&modules, 90);
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].related_module.as_deref(), Some("old-mod"));
    }

    #[test]
    fn test_detect_orphaned_rfcs() {
        let rfcs = vec![
            RfcInfo {
                rfc_id: "RFC-001".into(),
                title: "Add caching".into(),
                affected_modules: vec!["cache".into()],
            },
            RfcInfo {
                rfc_id: "RFC-002".into(),
                title: "Improve logging".into(),
                affected_modules: vec![],
            },
            RfcInfo {
                rfc_id: "RFC-003".into(),
                title: "Refactor auth".into(),
                affected_modules: vec!["nonexistent-module".into()],
            },
        ];
        let modules = vec![ModuleInfo {
            name: "cache".into(),
            what_it_does: Some("Caches things.".into()),
            last_updated: None,
            related_rfcs: vec!["RFC-001".into()],
        }];
        let gaps = detect_orphaned_rfcs(&rfcs, &modules);
        assert_eq!(gaps.len(), 2);
        assert!(gaps.iter().any(|g| g.related_rfc.as_deref() == Some("RFC-002")));
        assert!(gaps.iter().any(|g| g.related_rfc.as_deref() == Some("RFC-003")));
    }
}
