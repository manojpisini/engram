use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};

use engram_types::config::EngramConfig;

/// Shared Anthropic API client for all layer agents
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
    #[allow(dead_code)]
    usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Usage {
    #[allow(dead_code)]
    input_tokens: u32,
    #[allow(dead_code)]
    output_tokens: u32,
}

impl ClaudeClient {
    pub fn new(config: &EngramConfig) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            api_key: config.auth.anthropic_api_key.clone(),
            model: config.claude.model.clone(),
            max_tokens: config.claude.max_tokens,
        }
    }

    /// Send a prompt to Claude and get the text response.
    /// `system` is the system prompt defining the agent's role.
    /// `user_prompt` is the filled-in prompt template.
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
            let response = self
                .client
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
                error!("Claude API error ({}): {}", status, body);
                anyhow::bail!("Claude API call failed with status {status}: {body}");
            }

            let parsed: MessagesResponse =
                serde_json::from_str(&body).context("Failed to parse Claude response")?;

            let text = parsed
                .content
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

    /// Parse a JSON response from Claude, extracting structured data.
    /// On API failure, returns `T::default()` if the fallback string cannot
    /// be deserialized into `T`.
    pub async fn complete_json<T: serde::de::DeserializeOwned + Default>(
        &self,
        system: &str,
        user_prompt: &str,
    ) -> Result<T> {
        let raw = self.complete(system, user_prompt).await?;

        // Claude may wrap JSON in markdown code blocks — strip them
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

/// Extract JSON from a response that may be wrapped in markdown code blocks
fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();

    // Try to find JSON within code blocks
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

    // Try to find raw JSON object or array
    if let Some(start) = trimmed.find('{') {
        if let Some(end) = trimmed.rfind('}') {
            return &trimmed[start..=end];
        }
    }

    trimmed
}
