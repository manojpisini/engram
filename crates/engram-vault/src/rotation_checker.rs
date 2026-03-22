//! Secret rotation policy logic.
//!
//! Computes next rotation dates based on policy strings and checks whether
//! secrets are up-to-date, due soon, or overdue for rotation.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// Status of a secret's rotation relative to its policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RotationStatus {
    /// Secret is within its rotation window -- no action needed.
    Ok,
    /// Secret is due for rotation within the next 7 days.
    DueSoon,
    /// Secret has passed its rotation deadline.
    Overdue,
}

impl std::fmt::Display for RotationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RotationStatus::Ok => write!(f, "Ok"),
            RotationStatus::DueSoon => write!(f, "Due Soon"),
            RotationStatus::Overdue => write!(f, "Overdue"),
        }
    }
}

/// Compute the next rotation date given the last rotation timestamp and a policy string.
///
/// Supported policy strings:
/// - `"30d"` / `"30 days"` -- rotate every 30 days
/// - `"90d"` / `"90 days"` -- rotate every 90 days
/// - `"weekly"` -- rotate every 7 days
/// - `"monthly"` -- rotate every 30 days
/// - `"quarterly"` -- rotate every 90 days
/// - `"yearly"` / `"annually"` -- rotate every 365 days
/// - `"never"` -- returns a date far in the future (effectively no rotation)
///
/// Falls back to 90 days if the policy string is unrecognised.
pub fn compute_next_rotation(last_rotated: DateTime<Utc>, policy: &str) -> DateTime<Utc> {
    let days = parse_policy_days(policy);
    last_rotated + Duration::days(days)
}

/// Check rotation status for a secret whose next rotation is due at `next_due`.
///
/// - `Overdue` if `next_due` is in the past
/// - `DueSoon` if `next_due` is within the next 7 days
/// - `Ok` otherwise
pub fn check_rotation_status(next_due: DateTime<Utc>) -> RotationStatus {
    let now = Utc::now();
    if next_due <= now {
        RotationStatus::Overdue
    } else if next_due <= now + Duration::days(7) {
        RotationStatus::DueSoon
    } else {
        RotationStatus::Ok
    }
}

/// Compute how many days overdue a secret is. Returns 0 if not yet due.
pub fn days_overdue(next_due: DateTime<Utc>) -> i64 {
    let now = Utc::now();
    if next_due >= now {
        0
    } else {
        (now - next_due).num_days()
    }
}

/// Parse a rotation policy string into the number of days between rotations.
fn parse_policy_days(policy: &str) -> i64 {
    let policy = policy.trim().to_lowercase();

    // Try numeric formats: "30d", "30 days", "30"
    let numeric_part: String = policy.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !numeric_part.is_empty() {
        if let Ok(days) = numeric_part.parse::<i64>() {
            return days;
        }
    }

    // Named policies
    match policy.as_str() {
        "weekly" => 7,
        "monthly" => 30,
        "quarterly" => 90,
        "yearly" | "annually" => 365,
        "never" | "none" => 36500, // ~100 years
        _ => 90, // default fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_compute_next_rotation_30d() {
        let last = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let next = compute_next_rotation(last, "30d");
        assert_eq!(next, Utc.with_ymd_and_hms(2025, 1, 31, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_compute_next_rotation_quarterly() {
        let last = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let next = compute_next_rotation(last, "quarterly");
        assert_eq!(next, Utc.with_ymd_and_hms(2025, 4, 1, 0, 0, 0).unwrap());
    }

    #[test]
    fn test_check_rotation_overdue() {
        let past = Utc::now() - Duration::days(10);
        assert_eq!(check_rotation_status(past), RotationStatus::Overdue);
    }

    #[test]
    fn test_check_rotation_due_soon() {
        let soon = Utc::now() + Duration::days(3);
        assert_eq!(check_rotation_status(soon), RotationStatus::DueSoon);
    }

    #[test]
    fn test_check_rotation_ok() {
        let future = Utc::now() + Duration::days(30);
        assert_eq!(check_rotation_status(future), RotationStatus::Ok);
    }

    #[test]
    fn test_days_overdue_not_due() {
        let future = Utc::now() + Duration::days(10);
        assert_eq!(days_overdue(future), 0);
    }

    #[test]
    fn test_days_overdue_past() {
        let past = Utc::now() - Duration::days(5);
        assert!(days_overdue(past) >= 4); // allow for timing
    }

    #[test]
    fn test_parse_policy_never() {
        let last = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let next = compute_next_rotation(last, "never");
        // Should be ~100 years out
        assert!(next.format("%Y").to_string().parse::<i32>().unwrap() > 2100);
    }
}
