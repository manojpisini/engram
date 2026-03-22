use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tracing::{debug, error, warn};

use crate::config::EngramConfig;

const NOTION_API_BASE: &str = "https://api.notion.com/v1";
const NOTION_VERSION: &str = "2022-06-28";

// ═══════════════════════════════════════════════════════════════
//  Notion API Client
// ═══════════════════════════════════════════════════════════════

/// Notion REST API client shared by all ENGRAM agents.
#[derive(Clone)]
pub struct NotionClient {
    client: Client,
    token: String,
}

impl NotionClient {
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

        Self { client, token }
    }

    /// Query a database with optional filter, sorts, and pagination
    pub async fn query_database(
        &self,
        database_id: &str,
        filter: Option<Value>,
        sorts: Option<Value>,
        page_size: Option<u32>,
    ) -> Result<Value> {
        let mut payload = serde_json::json!({});
        if let Some(f) = filter {
            payload["filter"] = f;
        }
        if let Some(s) = sorts {
            payload["sorts"] = s;
        }
        if let Some(ps) = page_size {
            payload["page_size"] = serde_json::json!(ps);
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

    /// Create a page with body content blocks
    pub async fn create_page_with_content(
        &self,
        database_id: &str,
        properties: Value,
        children: Value,
    ) -> Result<Value> {
        let payload = serde_json::json!({
            "parent": { "database_id": database_id },
            "properties": properties,
            "children": children,
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
    pub async fn search(&self, query: &str) -> Result<Value> {
        let payload = serde_json::json!({ "query": query });
        self.post("/search", &payload).await
    }

    // ─── HTTP helpers ───

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

    async fn patch(&self, path: &str, payload: &Value) -> Result<Value> {
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
            if status.as_u16() == 429 {
                warn!("Notion rate limited on {method} {path}");
            }
            let snippet = &body[..body.len().min(300)];
            error!("Notion {method} {path} failed ({}): {}", status, snippet);
            anyhow::bail!("Notion {method} {path} failed: {status}");
        }

        serde_json::from_str(&body).context("Failed to parse Notion response")
    }
}

// ═══════════════════════════════════════════════════════════════
//  Claude API Client
// ═══════════════════════════════════════════════════════════════

/// Anthropic Messages API client shared by all ENGRAM agents.
#[derive(Clone)]
pub struct ClaudeClient {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

#[derive(Debug, Serialize)]
struct MessagesRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

impl ClaudeClient {
    pub fn new(config: &EngramConfig) -> Self {
        let api_key = if !config.auth.anthropic_api_key.is_empty() {
            config.auth.anthropic_api_key.clone()
        } else {
            std::env::var("ANTHROPIC_API_KEY").unwrap_or_default()
        };

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            api_key,
            model: config.claude.model.clone(),
            max_tokens: config.claude.max_tokens,
        }
    }

    /// Send a prompt to Claude and get the text response.
    /// On API failure (credit issues, network errors, etc.), returns a
    /// contextual fallback string so downstream agents keep running.
    pub async fn complete(&self, system: &str, user_prompt: &str) -> Result<String> {
        debug!("Claude API call (model: {})", self.model);

        let request = MessagesRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: vec![Message {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            }],
            system: Some(system.to_string()),
        };

        let result: Result<String> = async {
            let response = self.client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .json(&request)
                .send()
                .await
                .context("Failed to send Claude API request")?;

            let status = response.status();
            let body = response.text().await.context("Failed to read Claude response")?;

            if !status.is_success() {
                error!("Claude API error ({}): {}", status, &body[..body.len().min(300)]);
                anyhow::bail!("Claude API failed: {status}");
            }

            let parsed: MessagesResponse = serde_json::from_str(&body)
                .context("Failed to parse Claude response")?;

            let text = parsed.content
                .into_iter()
                .filter_map(|block| block.text)
                .collect::<Vec<_>>()
                .join("");

            Ok(text)
        }.await;

        match result {
            Ok(text) => Ok(text),
            Err(e) => {
                warn!("Claude API call failed, returning fallback response: {e:#}");
                Ok(Self::fallback_for_system_prompt(system))
            }
        }
    }

    /// Generate a contextual fallback string based on the system prompt.
    fn fallback_for_system_prompt(system: &str) -> String {
        let sys_lower = system.to_lowercase();

        if sys_lower.contains("narrative") || sys_lower.contains("health") {
            r#"{"narrative":"Data unavailable — Claude API is currently unreachable. Please retry later.","key_risks":["Unable to generate AI analysis at this time"],"key_wins":["System continued operating with fallback data"]}"#.to_string()
        } else if sys_lower.contains("review") {
            r#"{"review":"Automated review unavailable — Claude API is currently unreachable. Manual review recommended.","status":"fallback","issues":[]}"#.to_string()
        } else if sys_lower.contains("release") || sys_lower.contains("notes") {
            r#"{"release_notes":"Release notes generation unavailable — Claude API is currently unreachable.","version":"unknown","highlights":["Fallback: no AI-generated notes available"]}"#.to_string()
        } else {
            "Claude API is currently unreachable. This is a fallback response — no AI analysis was performed.".to_string()
        }
    }

    /// Parse a JSON response from Claude.
    /// On API failure, returns `T::default()` if `T` implements `Default`,
    /// otherwise propagates the error.
    pub async fn complete_json<T: serde::de::DeserializeOwned + Default>(
        &self,
        system: &str,
        user_prompt: &str,
    ) -> Result<T> {
        let raw = self.complete(system, user_prompt).await?;
        let json_str = extract_json(&raw);
        match serde_json::from_str::<T>(json_str) {
            Ok(val) => Ok(val),
            Err(e) => {
                warn!(
                    "Failed to parse Claude JSON response, returning T::default(): {e:#} — raw: {}",
                    &json_str[..json_str.len().min(200)]
                );
                Ok(T::default())
            }
        }
    }
}

fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }
    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        if let Some(end) = after.find("```") {
            return after[..end].trim();
        }
    }
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return &trimmed[start..=end];
        }
    }
    trimmed
}

// ═══════════════════════════════════════════════════════════════
//  Agent Context — passed to all agents
// ═══════════════════════════════════════════════════════════════

/// Shared context that agents can downcast from `Arc<dyn Any>`.
pub struct AgentContext {
    pub notion: NotionClient,
    pub claude: ClaudeClient,
    pub config: EngramConfig,
}

impl AgentContext {
    pub fn new(config: &EngramConfig) -> Self {
        Self {
            notion: NotionClient::new(config),
            claude: ClaudeClient::new(config),
            config: config.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Notion Property Builders
// ═══════════════════════════════════════════════════════════════

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
}
