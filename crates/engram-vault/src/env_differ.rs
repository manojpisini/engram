//! Three-way environment diff across dev, staging, and production.
//!
//! Compares env var presence across environments to detect drift,
//! missing variables, and value/sensitivity mismatches.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Represents an environment variable record from the Env Config database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    /// The variable name (e.g. `DATABASE_URL`).
    pub name: String,
    /// Which environment this record covers (dev / staging / prod).
    pub environment: String,
    /// Whether the var is present in this environment.
    pub present: bool,
    /// Sensitivity classification (e.g. "Secret", "Config", "Public").
    pub sensitivity: String,
    /// Last rotated timestamp (if applicable).
    pub last_rotated: Option<DateTime<Utc>>,
    /// Rotation policy string (e.g. "90d", "quarterly").
    pub rotation_policy: Option<String>,
}

/// A single mismatch between environments for a given variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvMismatch {
    /// The variable name.
    pub var_name: String,
    /// Description of the mismatch.
    pub detail: String,
}

/// Result of a three-way environment diff.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvDiffResult {
    /// Variables present in dev but missing in staging.
    pub missing_in_staging: Vec<String>,
    /// Variables present in dev but missing in production.
    pub missing_in_prod: Vec<String>,
    /// Variables present in staging but missing in production.
    pub staging_not_in_prod: Vec<String>,
    /// Variables with sensitivity or presence mismatches across environments.
    pub mismatches: Vec<EnvMismatch>,
    /// Orphaned variables: present in prod/staging but not referenced in dev.
    pub orphaned_in_prod: Vec<String>,
    /// Total variables examined.
    pub total_vars: usize,
    /// Snapshot timestamp.
    pub snapshot_at: DateTime<Utc>,
}

/// A config snapshot summary suitable for writing to the Config Snapshots database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSnapshot {
    /// The project identifier.
    pub project_id: String,
    /// Which environment was snapshot (or "cross-env" for a diff).
    pub environment: String,
    /// Total number of vars across all environments.
    pub total_vars: usize,
    /// Number of missing vars (union of missing in staging + prod).
    pub missing_vars: usize,
    /// Number of vars with mismatches.
    pub mismatch_count: usize,
    /// AI-generated notes (to be filled by the agent).
    pub ai_notes: String,
    /// When this snapshot was taken.
    pub snapshot_at: DateTime<Utc>,
}

/// Perform a three-way diff of environment variables across dev, staging, and production.
///
/// Each input slice represents all env var records for a given environment.
/// Variables are matched by name.
pub fn three_way_diff(dev: &[EnvVar], staging: &[EnvVar], prod: &[EnvVar]) -> EnvDiffResult {
    let dev_map = build_map(dev);
    let staging_map = build_map(staging);
    let prod_map = build_map(prod);

    let all_names: HashSet<&str> = dev_map
        .keys()
        .chain(staging_map.keys())
        .chain(prod_map.keys())
        .copied()
        .collect();

    let mut missing_in_staging = Vec::new();
    let mut missing_in_prod = Vec::new();
    let mut staging_not_in_prod = Vec::new();
    let mut mismatches = Vec::new();
    let mut orphaned_in_prod = Vec::new();

    for name in &all_names {
        let in_dev = dev_map.get(name).map(|v| v.present).unwrap_or(false);
        let in_staging = staging_map.get(name).map(|v| v.present).unwrap_or(false);
        let in_prod = prod_map.get(name).map(|v| v.present).unwrap_or(false);

        // Missing checks
        if in_dev && !in_staging {
            missing_in_staging.push(name.to_string());
        }
        if in_dev && !in_prod {
            missing_in_prod.push(name.to_string());
        }
        if in_staging && !in_prod && !in_dev {
            // Present in staging only, not in dev or prod
            staging_not_in_prod.push(name.to_string());
        } else if in_staging && !in_prod {
            staging_not_in_prod.push(name.to_string());
        }

        // Orphaned: in prod but not in dev (potential leftover)
        if in_prod && !in_dev {
            orphaned_in_prod.push(name.to_string());
        }

        // Sensitivity mismatches: compare across environments that have the var
        let sensitivities: Vec<(&str, &str)> = [
            ("dev", dev_map.get(name)),
            ("staging", staging_map.get(name)),
            ("prod", prod_map.get(name)),
        ]
        .iter()
        .filter_map(|(env, opt)| opt.map(|v| (*env, v.sensitivity.as_str())))
        .collect();

        if sensitivities.len() >= 2 {
            let first_sensitivity = sensitivities[0].1;
            for (env, sens) in &sensitivities[1..] {
                if *sens != first_sensitivity {
                    mismatches.push(EnvMismatch {
                        var_name: name.to_string(),
                        detail: format!(
                            "Sensitivity mismatch: {} has '{}' but {} has '{}'",
                            sensitivities[0].0, first_sensitivity, env, sens
                        ),
                    });
                    break; // one mismatch entry per var is sufficient
                }
            }
        }
    }

    // Sort for deterministic output
    missing_in_staging.sort();
    missing_in_prod.sort();
    staging_not_in_prod.sort();
    orphaned_in_prod.sort();

    EnvDiffResult {
        missing_in_staging,
        missing_in_prod,
        staging_not_in_prod,
        mismatches,
        orphaned_in_prod,
        total_vars: all_names.len(),
        snapshot_at: Utc::now(),
    }
}

/// Generate a ConfigSnapshot from a diff result.
pub fn generate_snapshot(project_id: &str, diff: &EnvDiffResult) -> ConfigSnapshot {
    ConfigSnapshot {
        project_id: project_id.to_string(),
        environment: "cross-env".to_string(),
        total_vars: diff.total_vars,
        missing_vars: diff.missing_in_staging.len() + diff.missing_in_prod.len(),
        mismatch_count: diff.mismatches.len(),
        ai_notes: String::new(), // To be filled by Claude analysis
        snapshot_at: diff.snapshot_at,
    }
}

/// Build a lookup map from var name to EnvVar (takes the first match if duplicates).
fn build_map(vars: &[EnvVar]) -> HashMap<&str, &EnvVar> {
    let mut map = HashMap::new();
    for var in vars {
        if var.present {
            map.entry(var.name.as_str()).or_insert(var);
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_var(name: &str, env: &str, present: bool, sensitivity: &str) -> EnvVar {
        EnvVar {
            name: name.to_string(),
            environment: env.to_string(),
            present,
            sensitivity: sensitivity.to_string(),
            last_rotated: None,
            rotation_policy: None,
        }
    }

    #[test]
    fn test_missing_in_prod() {
        let dev = vec![
            make_var("DATABASE_URL", "dev", true, "Secret"),
            make_var("API_KEY", "dev", true, "Secret"),
        ];
        let staging = vec![
            make_var("DATABASE_URL", "staging", true, "Secret"),
            make_var("API_KEY", "staging", true, "Secret"),
        ];
        let prod = vec![
            make_var("DATABASE_URL", "prod", true, "Secret"),
            // API_KEY missing in prod
        ];

        let result = three_way_diff(&dev, &staging, &prod);
        assert!(result.missing_in_prod.contains(&"API_KEY".to_string()));
        assert!(!result.missing_in_prod.contains(&"DATABASE_URL".to_string()));
    }

    #[test]
    fn test_sensitivity_mismatch() {
        let dev = vec![make_var("TOKEN", "dev", true, "Secret")];
        let staging = vec![make_var("TOKEN", "staging", true, "Config")];
        let prod = vec![make_var("TOKEN", "prod", true, "Secret")];

        let result = three_way_diff(&dev, &staging, &prod);
        assert!(!result.mismatches.is_empty());
        assert_eq!(result.mismatches[0].var_name, "TOKEN");
    }

    #[test]
    fn test_orphaned_in_prod() {
        let dev = vec![make_var("API_KEY", "dev", true, "Secret")];
        let staging = vec![make_var("API_KEY", "staging", true, "Secret")];
        let prod = vec![
            make_var("API_KEY", "prod", true, "Secret"),
            make_var("OLD_LEGACY_VAR", "prod", true, "Config"),
        ];

        let result = three_way_diff(&dev, &staging, &prod);
        assert!(result.orphaned_in_prod.contains(&"OLD_LEGACY_VAR".to_string()));
    }

    #[test]
    fn test_all_in_sync() {
        let dev = vec![make_var("DB", "dev", true, "Secret")];
        let staging = vec![make_var("DB", "staging", true, "Secret")];
        let prod = vec![make_var("DB", "prod", true, "Secret")];

        let result = three_way_diff(&dev, &staging, &prod);
        assert!(result.missing_in_staging.is_empty());
        assert!(result.missing_in_prod.is_empty());
        assert!(result.mismatches.is_empty());
        assert!(result.orphaned_in_prod.is_empty());
    }

    #[test]
    fn test_generate_snapshot() {
        let dev = vec![make_var("A", "dev", true, "Secret")];
        let staging = vec![];
        let prod = vec![];

        let diff = three_way_diff(&dev, &staging, &prod);
        let snapshot = generate_snapshot("proj-1", &diff);
        assert_eq!(snapshot.project_id, "proj-1");
        assert_eq!(snapshot.total_vars, 1);
        assert!(snapshot.missing_vars > 0);
    }
}
