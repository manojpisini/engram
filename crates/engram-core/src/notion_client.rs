use anyhow::{Context, Result};
use reqwest::Client;
use serde::Serialize;
use serde_json::Value;
use tracing::{debug, error, warn};

use engram_types::config::EngramConfig;

const NOTION_API_BASE: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";

/// Notion API client for ENGRAM.
/// Wraps the Notion REST API with typed helpers for all database operations.
#[derive(Clone)]
pub struct NotionMcpClient {
    client: Client,
    token: String,
    #[allow(dead_code)]
    workspace_id: String,
}

/// Filter condition for database queries (public API for agents and setup endpoints)
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct QueryFilter {
    pub property: String,
    pub condition: FilterCondition,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "value")]
#[allow(dead_code)]
pub enum FilterCondition {
    #[serde(rename = "equals")]
    Equals(Value),
    #[serde(rename = "contains")]
    Contains(String),
    #[serde(rename = "greater_than")]
    GreaterThan(Value),
    #[serde(rename = "less_than")]
    LessThan(Value),
    #[serde(rename = "is_not_empty")]
    IsNotEmpty,
    #[serde(rename = "is_empty")]
    IsEmpty,
    #[serde(rename = "checkbox_equals")]
    CheckboxEquals(bool),
}

/// Sort direction for query results
#[derive(Debug, Clone, Serialize)]
pub struct QuerySort {
    pub property: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Serialize)]
pub enum SortDirection {
    #[serde(rename = "ascending")]
    Ascending,
    #[serde(rename = "descending")]
    Descending,
}

impl NotionMcpClient {
    pub fn new(config: &EngramConfig) -> Self {
        let token = if !config.auth.notion_mcp_token.is_empty() {
            config.auth.notion_mcp_token.clone()
        } else {
            std::env::var("NOTION_MCP_TOKEN").unwrap_or_default()
        };

        if token.is_empty() {
            warn!("NOTION_MCP_TOKEN is empty — Notion API calls will fail");
        }

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            token,
            workspace_id: config.workspace.notion_workspace_id.clone(),
        }
    }

    /// Create a new Notion database under a parent page
    pub async fn create_database(
        &self,
        title: &str,
        parent_page_id: &str,
        properties: Value,
    ) -> Result<Value> {
        let payload = serde_json::json!({
            "parent": { "type": "page_id", "page_id": parent_page_id },
            "title": [{ "type": "text", "text": { "content": title } }],
            "properties": properties,
        });

        self.post("/databases", &payload).await
    }

    /// Retrieve a database schema
    pub async fn retrieve_database(&self, database_id: &str) -> Result<Value> {
        self.get(&format!("/databases/{database_id}")).await
    }

    /// Query a database with optional filter, sorts, and pagination
    pub async fn query_database(
        &self,
        database_id: &str,
        filter: Option<Value>,
        sorts: Option<Vec<QuerySort>>,
        page_size: Option<u32>,
        start_cursor: Option<&str>,
    ) -> Result<Value> {
        let mut payload = serde_json::json!({});

        if let Some(f) = filter {
            payload["filter"] = f;
        }
        if let Some(s) = sorts {
            payload["sorts"] = serde_json::to_value(s)?;
        }
        if let Some(ps) = page_size {
            payload["page_size"] = serde_json::json!(ps);
        }
        if let Some(sc) = start_cursor {
            payload["start_cursor"] = serde_json::json!(sc);
        }

        self.post(&format!("/databases/{database_id}/query"), &payload).await
    }

    /// Create a new page (row) in a database
    pub async fn create_page(
        &self,
        database_id: &str,
        properties: Value,
    ) -> Result<Value> {
        let payload = serde_json::json!({
            "parent": { "database_id": database_id },
            "properties": properties,
        });

        self.post("/pages", &payload).await
    }

    /// Update an existing page's properties
    pub async fn update_page(
        &self,
        page_id: &str,
        properties: Value,
    ) -> Result<Value> {
        let payload = serde_json::json!({
            "properties": properties,
        });

        self.patch(&format!("/pages/{page_id}"), &payload).await
    }

    /// Search across the workspace
    pub async fn search(&self, query: &str, filter: Option<Value>) -> Result<Value> {
        let mut payload = serde_json::json!({
            "query": query,
        });
        if let Some(f) = filter {
            payload["filter"] = f;
        }

        self.post("/search", &payload).await
    }

    /// Archive (soft-delete) a page
    pub async fn archive_page(&self, page_id: &str) -> Result<Value> {
        let payload = serde_json::json!({
            "archived": true,
        });
        self.patch(&format!("/pages/{page_id}"), &payload).await
    }

    /// Append blocks (rich content) to a page
    pub async fn append_blocks(&self, page_id: &str, children: Value) -> Result<Value> {
        let payload = serde_json::json!({
            "children": children,
        });
        self.patch(&format!("/blocks/{page_id}/children"), &payload).await
    }

    // ─── HTTP helpers ───

    async fn get(&self, path: &str) -> Result<Value> {
        let url = format!("{NOTION_API_BASE}{path}");
        debug!("Notion GET {}", path);

        let response = self.client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", NOTION_VERSION)
            .send()
            .await
            .context("Failed to send Notion API request")?;

        self.handle_response(response, "GET", path).await
    }

    async fn post(&self, path: &str, payload: &Value) -> Result<Value> {
        let url = format!("{NOTION_API_BASE}{path}");
        debug!("Notion POST {}", path);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", NOTION_VERSION)
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
            .context("Failed to send Notion API request")?;

        self.handle_response(response, "POST", path).await
    }

    pub async fn patch(&self, path: &str, payload: &Value) -> Result<Value> {
        let url = format!("{NOTION_API_BASE}{path}");
        debug!("Notion PATCH {}", path);

        let response = self.client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Notion-Version", NOTION_VERSION)
            .header("Content-Type", "application/json")
            .json(payload)
            .send()
            .await
            .context("Failed to send Notion API request")?;

        self.handle_response(response, "PATCH", path).await
    }

    async fn handle_response(
        &self,
        response: reqwest::Response,
        method: &str,
        path: &str,
    ) -> Result<Value> {
        let status = response.status();
        let body = response.text().await.context("Failed to read response body")?;

        if !status.is_success() {
            // Rate limit handling
            if status.as_u16() == 429 {
                warn!("Notion rate limited on {method} {path} — consider adding retry logic");
            }
            error!("Notion API {method} {path} failed ({}): {}", status, &body[..body.len().min(500)]);
            anyhow::bail!("Notion API {method} {path} failed with status {status}");
        }

        let result: Value = serde_json::from_str(&body)
            .context("Failed to parse Notion API response JSON")?;

        Ok(result)
    }
}

/// Helper to build Notion property values for create-page / update-page.
/// Used by init, setup endpoints, and layer agents.
#[allow(unused)]
pub mod properties {
    use serde_json::{json, Value};

    pub fn title(text: &str) -> Value {
        json!({ "title": [{ "text": { "content": text } }] })
    }

    pub fn rich_text(text: &str) -> Value {
        json!({ "rich_text": [{ "text": { "content": text } }] })
    }

    pub fn number(n: f64) -> Value {
        json!({ "number": n })
    }

    pub fn select(name: &str) -> Value {
        json!({ "select": { "name": name } })
    }

    pub fn multi_select(names: &[&str]) -> Value {
        let opts: Vec<Value> = names.iter().map(|n| json!({ "name": n })).collect();
        json!({ "multi_select": opts })
    }

    pub fn checkbox(checked: bool) -> Value {
        json!({ "checkbox": checked })
    }

    pub fn date(start: &str) -> Value {
        json!({ "date": { "start": start } })
    }

    pub fn url(url: &str) -> Value {
        json!({ "url": url })
    }

    pub fn relation(page_ids: &[&str]) -> Value {
        let rels: Vec<Value> = page_ids.iter().map(|id| json!({ "id": id })).collect();
        json!({ "relation": rels })
    }

    pub fn email(email: &str) -> Value {
        json!({ "email": email })
    }

    pub fn person(user_ids: &[&str]) -> Value {
        let people: Vec<Value> = user_ids
            .iter()
            .map(|id| json!({ "object": "user", "id": id }))
            .collect();
        json!({ "people": people })
    }
}
