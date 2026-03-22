use anyhow::{bail, Result};
use engram_types::events::RfcStatus;
use tracing::info;

/// Returns the list of valid next statuses for a given RFC status.
fn allowed_transitions(from: RfcStatus) -> &'static [RfcStatus] {
    match from {
        RfcStatus::Draft => &[RfcStatus::UnderReview],
        RfcStatus::UnderReview => &[RfcStatus::Approved, RfcStatus::Deprecated],
        RfcStatus::Approved => &[RfcStatus::Implementing, RfcStatus::Deprecated],
        RfcStatus::Implementing => &[RfcStatus::Implemented, RfcStatus::Deprecated],
        RfcStatus::Implemented => &[RfcStatus::Deprecated],
        RfcStatus::Deprecated => &[],
    }
}

/// Validate and execute an RFC status transition via the Notion MCP.
///
/// Currently logs what it would do; actual MCP calls will be wired up later.
pub async fn transition_rfc_status(
    page_id: &str,
    from: RfcStatus,
    to: RfcStatus,
) -> Result<()> {
    let allowed = allowed_transitions(from);
    if !allowed.contains(&to) {
        bail!(
            "Invalid RFC transition: {from} -> {to}. Allowed targets from {from}: {:?}",
            allowed.iter().map(|s| s.to_string()).collect::<Vec<_>>()
        );
    }

    info!(
        "[DecisionsAgent] Would call MCP: notion.update_page with {{ page_id: \"{page_id}\", \
         properties: {{ \"Status\": \"{to}\" }} }}"
    );

    Ok(())
}

/// Flag an RFC as stale by adding a "Stale" tag.
///
/// Currently logs what it would do; actual MCP calls will be wired up later.
pub async fn flag_rfc_as_stale(page_id: &str) -> Result<()> {
    info!(
        "[DecisionsAgent] Would call MCP: notion.update_page with {{ page_id: \"{page_id}\", \
         properties: {{ \"Tags\": [\"Stale\"] }} }}"
    );
    Ok(())
}

/// Create a new RFC draft page in Notion.
///
/// Currently logs what it would do; actual MCP calls will be wired up later.
pub async fn create_rfc_draft(
    project_id: &str,
    title: &str,
    body: &str,
) -> Result<String> {
    let placeholder_page_id = format!("draft-{}", &title.replace(' ', "-").to_lowercase());

    info!(
        "[DecisionsAgent] Would call MCP: notion.create_page with {{ \
         database: \"RFCs\", \
         project_id: \"{project_id}\", \
         title: \"{title}\", \
         status: \"Draft\", \
         body_length: {} }}",
        body.len()
    );

    Ok(placeholder_page_id)
}

/// Query Notion for RFCs in "Under Review" status that are older than the given
/// number of days.
///
/// Currently logs what it would do and returns an empty list.
pub async fn query_stale_rfcs(
    project_id: &str,
    older_than_days: i64,
) -> Result<Vec<StaleRfc>> {
    info!(
        "[DecisionsAgent] Would call MCP: notion.query_database with {{ \
         database: \"RFCs\", \
         filter: {{ \"Status\": \"Under Review\", \"created_before_days\": {older_than_days} }}, \
         project_id: \"{project_id}\" }}"
    );

    // Return empty for now; real implementation will parse Notion response.
    Ok(Vec::new())
}

/// Represents a stale RFC returned from a Notion query.
#[derive(Debug, Clone)]
pub struct StaleRfc {
    pub page_id: String,
    pub title: String,
    pub days_in_review: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn valid_transition_draft_to_under_review() {
        let result = transition_rfc_status("page-1", RfcStatus::Draft, RfcStatus::UnderReview).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invalid_transition_draft_to_approved() {
        let result = transition_rfc_status("page-1", RfcStatus::Draft, RfcStatus::Approved).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn valid_transition_approved_to_implementing() {
        let result = transition_rfc_status("page-1", RfcStatus::Approved, RfcStatus::Implementing).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn deprecated_is_terminal() {
        let result = transition_rfc_status("page-1", RfcStatus::Deprecated, RfcStatus::Draft).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn any_status_can_deprecate() {
        for from in [
            RfcStatus::UnderReview,
            RfcStatus::Approved,
            RfcStatus::Implementing,
            RfcStatus::Implemented,
        ] {
            let result = transition_rfc_status("page-1", from, RfcStatus::Deprecated).await;
            assert!(result.is_ok(), "Should allow {from} -> Deprecated");
        }
    }
}
