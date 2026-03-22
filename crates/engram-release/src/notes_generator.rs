//! Release notes and migration notes generation.

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Structured release notes parsed from Claude response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseNotes {
    pub features: Vec<String>,
    pub fixes: Vec<String>,
    pub performance: Vec<String>,
    pub security: Vec<String>,
    pub breaking_changes: Vec<String>,
    pub summary: String,
}

/// Structured migration notes parsed from Claude response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationNotes {
    pub before_upgrade: Vec<String>,
    pub env_changes: Vec<EnvChange>,
    pub breaking_migration: Vec<BreakingMigration>,
    pub dependency_notes: Vec<String>,
    pub verification: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvChange {
    pub var: String,
    pub description: String,
    pub example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BreakingMigration {
    pub change: String,
    pub steps: Vec<String>,
}

/// Release readiness assessment from Claude.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadinessAssessment {
    pub release_ready: bool,
    pub blockers: Vec<String>,
    pub risks: Vec<String>,
    pub recommendation: String,
}

/// Parse Claude's release notes JSON response.
pub fn parse_release_notes(response: &str) -> Result<ReleaseNotes> {
    let json_str = extract_json(response);
    let notes: ReleaseNotes = serde_json::from_str(json_str)?;
    Ok(notes)
}

/// Parse Claude's migration notes JSON response.
pub fn parse_migration_notes(response: &str) -> Result<MigrationNotes> {
    let json_str = extract_json(response);
    let notes: MigrationNotes = serde_json::from_str(json_str)?;
    Ok(notes)
}

/// Parse Claude's readiness assessment JSON response.
pub fn parse_readiness_assessment(response: &str) -> Result<ReadinessAssessment> {
    let json_str = extract_json(response);
    let assessment: ReadinessAssessment = serde_json::from_str(json_str)?;
    Ok(assessment)
}

/// Format release notes as Markdown for the Notion Release record.
pub fn format_release_notes_markdown(notes: &ReleaseNotes) -> String {
    let mut md = String::new();
    md.push_str(&format!("## {}\n\n", notes.summary));

    if !notes.features.is_empty() {
        md.push_str("### Features\n");
        for f in &notes.features {
            md.push_str(&format!("- {f}\n"));
        }
        md.push('\n');
    }
    if !notes.fixes.is_empty() {
        md.push_str("### Bug Fixes\n");
        for f in &notes.fixes {
            md.push_str(&format!("- {f}\n"));
        }
        md.push('\n');
    }
    if !notes.performance.is_empty() {
        md.push_str("### Performance\n");
        for p in &notes.performance {
            md.push_str(&format!("- {p}\n"));
        }
        md.push('\n');
    }
    if !notes.security.is_empty() {
        md.push_str("### Security\n");
        for s in &notes.security {
            md.push_str(&format!("- {s}\n"));
        }
        md.push('\n');
    }
    if !notes.breaking_changes.is_empty() {
        md.push_str("### Breaking Changes\n");
        for b in &notes.breaking_changes {
            md.push_str(&format!("- {b}\n"));
        }
        md.push('\n');
    }
    md
}

/// Format migration notes as Markdown.
pub fn format_migration_notes_markdown(notes: &MigrationNotes) -> String {
    let mut md = String::from("## Migration Guide\n\n");

    if !notes.before_upgrade.is_empty() {
        md.push_str("### Before Upgrading\n");
        for step in &notes.before_upgrade {
            md.push_str(&format!("1. {step}\n"));
        }
        md.push('\n');
    }
    if !notes.env_changes.is_empty() {
        md.push_str("### New Environment Variables\n");
        for env in &notes.env_changes {
            md.push_str(&format!(
                "- `{}`: {} (example: `{}`)\n",
                env.var, env.description, env.example
            ));
        }
        md.push('\n');
    }
    if !notes.breaking_migration.is_empty() {
        md.push_str("### Breaking Change Migration\n");
        for brk in &notes.breaking_migration {
            md.push_str(&format!("**{}**\n", brk.change));
            for (i, step) in brk.steps.iter().enumerate() {
                md.push_str(&format!("{}. {step}\n", i + 1));
            }
            md.push('\n');
        }
    }
    if !notes.verification.is_empty() {
        md.push_str("### Verification\n");
        for v in &notes.verification {
            md.push_str(&format!("- [ ] {v}\n"));
        }
        md.push('\n');
    }
    md
}

/// Extract JSON from a Claude response that may contain surrounding text.
fn extract_json(text: &str) -> &str {
    let text = text.trim();
    if let Some(start) = text.find('{') {
        if let Some(end) = text.rfind('}') {
            return &text[start..=end];
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_release_notes() {
        let json = r#"{
            "features": ["Added new caching layer (#42)"],
            "fixes": ["Fixed memory leak in worker pool (#38)"],
            "performance": [],
            "security": ["Updated openssl to 3.1.4 (CVE-2024-1234)"],
            "breaking_changes": [],
            "summary": "Adds caching and fixes critical memory leak."
        }"#;
        let notes = parse_release_notes(json).unwrap();
        assert_eq!(notes.features.len(), 1);
        assert_eq!(notes.fixes.len(), 1);
        assert!(notes.breaking_changes.is_empty());
    }

    #[test]
    fn test_format_release_notes() {
        let notes = ReleaseNotes {
            features: vec!["New API endpoint".into()],
            fixes: vec![],
            performance: vec![],
            security: vec![],
            breaking_changes: vec![],
            summary: "Minor feature release.".into(),
        };
        let md = format_release_notes_markdown(&notes);
        assert!(md.contains("### Features"));
        assert!(md.contains("New API endpoint"));
        assert!(!md.contains("### Bug Fixes"));
    }

    #[test]
    fn test_parse_readiness() {
        let json = r#"{"release_ready": false, "blockers": ["Open critical regression"], "risks": [], "recommendation": "Hold until regression resolved"}"#;
        let assessment = parse_readiness_assessment(json).unwrap();
        assert!(!assessment.release_ready);
        assert_eq!(assessment.blockers.len(), 1);
    }
}
