//! Claude prompt templates for the Health agent.

/// System prompt for health narrative generation.
pub const HEALTH_NARRATIVE_SYSTEM: &str = r#"You are ENGRAM's Health Intelligence engine. Your job is to analyze weekly health metrics across all six engineering layers (Decisions, Pulse, Shield, Atlas, Vault, Review) and produce a concise, actionable health narrative for the engineering team.

You will receive structured health data including per-layer scores, deltas, and weekly statistics. Respond with a JSON object containing exactly these fields:

- "ai_narrative": A 2-4 paragraph narrative summarizing the overall health trajectory, highlighting the most important trends and their engineering implications.
- "key_risks": An array of 1-5 strings, each describing a concrete risk that needs attention this week.
- "key_wins": An array of 1-5 strings, each describing a positive achievement or improvement worth celebrating.

Respond ONLY with the JSON object, no surrounding text."#;

/// Generates the user prompt for health narrative with all layer scores and stats.
pub fn health_narrative_prompt(
    project_id: &str,
    period: &str,
    decisions_health: f64,
    pulse_health: f64,
    shield_health: f64,
    atlas_health: f64,
    vault_health: f64,
    review_health: f64,
    overall_score: f64,
    delta_from_last: f64,
    digest_summary: &str,
) -> String {
    format!(
        r#"Generate a weekly health narrative for project "{project_id}" covering period "{period}".

## Health Scores (0-100)
- Decisions (RFC governance): {decisions_health:.1}
- Pulse (Performance benchmarks): {pulse_health:.1}
- Shield (Security/CVE posture): {shield_health:.1}
- Atlas (Knowledge documentation): {atlas_health:.1}
- Vault (Secret/config management): {vault_health:.1}
- Review (Code review quality): {review_health:.1}
- **Overall**: {overall_score:.1} (delta from last week: {delta_from_last:+.1})

## Weekly Digest Summary
{digest_summary}

Analyze these metrics and produce the health narrative JSON."#
    )
}
