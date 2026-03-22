//! Health score computation formulas for each ENGRAM layer.
//!
//! Each function returns a score in the range [0.0, 100.0].

/// Decisions Health = (1 - open_stale_rfcs / total_rfcs) * 100
///
/// Returns 100.0 when there are no RFCs (nothing stale).
pub fn compute_decisions_health(open_stale: u32, total: u32) -> f64 {
    if total == 0 {
        return 100.0;
    }
    let ratio = open_stale as f64 / total as f64;
    ((1.0 - ratio) * 100.0).clamp(0.0, 100.0)
}

/// Pulse Health = (normal_benchmarks / total_benchmarks) * 100
///
/// Returns 100.0 when there are no benchmarks.
pub fn compute_pulse_health(normal: u32, total: u32) -> f64 {
    if total == 0 {
        return 100.0;
    }
    (normal as f64 / total as f64 * 100.0).clamp(0.0, 100.0)
}

/// Shield Health = (triaged_vulns / total_vulns) * 100
///
/// Weighted by severity: Critical=4, High=2, Medium=1, Low=0.5.
/// Returns 100.0 when there are no vulnerabilities.
pub fn compute_shield_health(
    triaged_critical: u32,
    total_critical: u32,
    triaged_high: u32,
    total_high: u32,
    triaged_medium: u32,
    total_medium: u32,
    triaged_low: u32,
    total_low: u32,
) -> f64 {
    let weighted_triaged = triaged_critical as f64 * 4.0
        + triaged_high as f64 * 2.0
        + triaged_medium as f64 * 1.0
        + triaged_low as f64 * 0.5;

    let weighted_total = total_critical as f64 * 4.0
        + total_high as f64 * 2.0
        + total_medium as f64 * 1.0
        + total_low as f64 * 0.5;

    if weighted_total == 0.0 {
        return 100.0;
    }
    (weighted_triaged / weighted_total * 100.0).clamp(0.0, 100.0)
}

/// Atlas Health = (documented_modules / total_modules) * 100 * (1 - open_gaps / 10)
///
/// The open_gaps penalty factor is clamped so it cannot go below 0.
/// Returns 100.0 when there are no modules.
pub fn compute_atlas_health(documented: u32, total: u32, open_gaps: u32) -> f64 {
    if total == 0 {
        return 100.0;
    }
    let coverage = documented as f64 / total as f64 * 100.0;
    let gap_penalty = (1.0 - open_gaps as f64 / 10.0).max(0.0);
    (coverage * gap_penalty).clamp(0.0, 100.0)
}

/// Vault Health = (valid_secrets / total_secrets) * 100, penalized for missing-in-prod.
///
/// Each missing-in-prod secret reduces the score by 5 points.
/// Returns 100.0 when there are no secrets.
pub fn compute_vault_health(valid: u32, total: u32, missing_in_prod: u32) -> f64 {
    if total == 0 {
        return 100.0;
    }
    let base = valid as f64 / total as f64 * 100.0;
    let penalty = missing_in_prod as f64 * 5.0;
    (base - penalty).clamp(0.0, 100.0)
}

/// Review Health = (reviewed_prs / merged_prs) * 100 * (1 - critical_patterns / 10)
///
/// The critical_patterns penalty is clamped so it cannot go below 0.
/// Returns 100.0 when there are no merged PRs.
pub fn compute_review_health(reviewed: u32, merged: u32, critical_patterns: u32) -> f64 {
    if merged == 0 {
        return 100.0;
    }
    let review_rate = reviewed as f64 / merged as f64 * 100.0;
    let pattern_penalty = (1.0 - critical_patterns as f64 / 10.0).max(0.0);
    (review_rate * pattern_penalty).clamp(0.0, 100.0)
}

/// Overall Health = 0.2*D + 0.2*Pu + 0.2*S + 0.1*A + 0.15*V + 0.15*R
pub fn compute_overall(d: f64, pu: f64, s: f64, a: f64, v: f64, r: f64) -> f64 {
    let score = 0.20 * d + 0.20 * pu + 0.20 * s + 0.10 * a + 0.15 * v + 0.15 * r;
    score.clamp(0.0, 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decisions_health_no_rfcs() {
        assert_eq!(compute_decisions_health(0, 0), 100.0);
    }

    #[test]
    fn test_decisions_health_none_stale() {
        assert_eq!(compute_decisions_health(0, 10), 100.0);
    }

    #[test]
    fn test_decisions_health_all_stale() {
        assert_eq!(compute_decisions_health(10, 10), 0.0);
    }

    #[test]
    fn test_decisions_health_half_stale() {
        assert!((compute_decisions_health(5, 10) - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_pulse_health_no_benchmarks() {
        assert_eq!(compute_pulse_health(0, 0), 100.0);
    }

    #[test]
    fn test_pulse_health_all_normal() {
        assert_eq!(compute_pulse_health(20, 20), 100.0);
    }

    #[test]
    fn test_pulse_health_half_normal() {
        assert!((compute_pulse_health(10, 20) - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_shield_health_no_vulns() {
        assert_eq!(compute_shield_health(0, 0, 0, 0, 0, 0, 0, 0), 100.0);
    }

    #[test]
    fn test_shield_health_all_triaged() {
        assert_eq!(compute_shield_health(2, 2, 3, 3, 5, 5, 10, 10), 100.0);
    }

    #[test]
    fn test_shield_health_none_triaged() {
        assert_eq!(compute_shield_health(0, 2, 0, 3, 0, 5, 0, 10), 0.0);
    }

    #[test]
    fn test_shield_health_weighted() {
        // Only critical triaged: triaged_weight = 2*4 = 8, total_weight = 2*4 + 2*2 = 12
        let score = compute_shield_health(2, 2, 0, 2, 0, 0, 0, 0);
        let expected = 8.0 / 12.0 * 100.0;
        assert!((score - expected).abs() < 0.01);
    }

    #[test]
    fn test_atlas_health_no_modules() {
        assert_eq!(compute_atlas_health(0, 0, 0), 100.0);
    }

    #[test]
    fn test_atlas_health_full_coverage_no_gaps() {
        assert_eq!(compute_atlas_health(10, 10, 0), 100.0);
    }

    #[test]
    fn test_atlas_health_with_gaps() {
        // coverage = 100%, gap_penalty = 1 - 5/10 = 0.5 => 50.0
        assert!((compute_atlas_health(10, 10, 5) - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_atlas_health_gaps_exceed_10() {
        // gap_penalty clamped to 0.0
        assert_eq!(compute_atlas_health(10, 10, 15), 0.0);
    }

    #[test]
    fn test_vault_health_no_secrets() {
        assert_eq!(compute_vault_health(0, 0, 0), 100.0);
    }

    #[test]
    fn test_vault_health_all_valid_no_missing() {
        assert_eq!(compute_vault_health(10, 10, 0), 100.0);
    }

    #[test]
    fn test_vault_health_with_missing_prod() {
        // base = 100%, penalty = 2*5 = 10 => 90.0
        assert!((compute_vault_health(10, 10, 2) - 90.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_vault_health_clamps_to_zero() {
        // base = 50%, penalty = 20*5 = 100 => clamped to 0.0
        assert_eq!(compute_vault_health(5, 10, 20), 0.0);
    }

    #[test]
    fn test_review_health_no_merged() {
        assert_eq!(compute_review_health(0, 0, 0), 100.0);
    }

    #[test]
    fn test_review_health_all_reviewed_no_patterns() {
        assert_eq!(compute_review_health(10, 10, 0), 100.0);
    }

    #[test]
    fn test_review_health_with_critical_patterns() {
        // review_rate = 100%, pattern_penalty = 1 - 3/10 = 0.7 => 70.0
        assert!((compute_review_health(10, 10, 3) - 70.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_review_health_patterns_exceed_10() {
        // pattern_penalty clamped to 0.0
        assert_eq!(compute_review_health(10, 10, 15), 0.0);
    }

    #[test]
    fn test_overall_all_perfect() {
        let score = compute_overall(100.0, 100.0, 100.0, 100.0, 100.0, 100.0);
        assert!((score - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_overall_all_zero() {
        assert_eq!(compute_overall(0.0, 0.0, 0.0, 0.0, 0.0, 0.0), 0.0);
    }

    #[test]
    fn test_overall_weighted() {
        // D=50, Pu=50, S=50, A=50, V=50, R=50
        // 0.2*50 + 0.2*50 + 0.2*50 + 0.1*50 + 0.15*50 + 0.15*50 = 50
        let score = compute_overall(50.0, 50.0, 50.0, 50.0, 50.0, 50.0);
        assert!((score - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_overall_asymmetric_weights() {
        // Only Decisions = 100, rest = 0
        // 0.2*100 = 20.0
        let score = compute_overall(100.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        assert!((score - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_overall_atlas_weight() {
        // Only Atlas = 100 => 0.1*100 = 10.0
        let score = compute_overall(0.0, 0.0, 0.0, 100.0, 0.0, 0.0);
        assert!((score - 10.0).abs() < f64::EPSILON);
    }
}
