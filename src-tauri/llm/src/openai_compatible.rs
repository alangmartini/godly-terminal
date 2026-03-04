use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::branch_name::sanitize_branch_name;

const OPENAI_COMPAT_API_BASE: &str = "https://api.openai.com/v1/chat/completions";

const SYSTEM_PROMPT: &str = "\
You are a git branch name generator. Given a description of a task, output ONLY a short, \
kebab-case branch name. Rules:\n\
- Use lowercase letters, numbers, and hyphens only\n\
- Start with a conventional prefix: feat/, fix/, refactor/, docs/, chore/, test/\n\
- Keep it under 50 characters total\n\
- No explanations, just the branch name";

/// Maximum number of retries for rate-limited (429) requests.
const MAX_RETRIES: u32 = 3;

#[derive(Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<OpenAiMessage>,
    temperature: f32,
    max_tokens: u32,
}

#[derive(Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    choices: Option<Vec<OpenAiChoice>>,
    error: Option<OpenAiError>,
}

#[derive(Deserialize)]
struct OpenAiChoice {
    message: Option<OpenAiResponseMessage>,
    text: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiResponseMessage {
    content: Option<OpenAiContent>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OpenAiContent {
    Text(String),
    Parts(Vec<OpenAiContentPart>),
}

#[derive(Deserialize)]
struct OpenAiContentPart {
    #[serde(rename = "type")]
    kind: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OpenAiError {
    Structured { message: String },
    Plain(String),
}

impl OpenAiError {
    fn message(&self) -> &str {
        match self {
            OpenAiError::Structured { message } => message,
            OpenAiError::Plain(s) => s,
        }
    }
}

fn extract_text(body: &OpenAiResponse) -> Option<String> {
    body.choices
        .as_ref()
        .and_then(|choices| choices.first())
        .and_then(|choice| {
            choice
                .message
                .as_ref()
                .and_then(|message| message.content.as_ref())
                .and_then(|content| match content {
                    OpenAiContent::Text(text) => Some(text.clone()),
                    OpenAiContent::Parts(parts) => parts.iter().find_map(|part| {
                        if matches!(part.kind.as_deref(), Some("text") | None) {
                            part.text.clone()
                        } else {
                            None
                        }
                    }),
                })
                .or_else(|| choice.text.clone())
        })
}

/// Generate a branch name via an OpenAI-compatible chat-completions endpoint.
///
/// `api_base_url` defaults to OpenAI's `/v1/chat/completions` URL, but can point
/// to any compatible provider endpoint.
pub async fn generate_branch_name_openai_compatible(
    api_key: &str,
    description: &str,
    model: &str,
    api_base_url: Option<&str>,
) -> Result<String> {
    let client = reqwest::Client::new();
    let endpoint = api_base_url
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .unwrap_or(OPENAI_COMPAT_API_BASE);

    let request = OpenAiRequest {
        model: model.trim().to_string(),
        messages: vec![
            OpenAiMessage {
                role: "system".to_string(),
                content: SYSTEM_PROMPT.to_string(),
            },
            OpenAiMessage {
                role: "user".to_string(),
                content: description.to_string(),
            },
        ],
        temperature: 0.3,
        max_tokens: 200,
    };

    let mut last_error = None;
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
            tokio::time::sleep(delay).await;
        }

        let response = client
            .post(endpoint)
            .bearer_auth(api_key)
            .json(&request)
            .send()
            .await
            .context("Failed to call OpenAI-compatible API")?;

        let status = response.status();
        if status.as_u16() == 429 {
            last_error = Some(format!("Rate limited (429) on attempt {}", attempt + 1));
            continue;
        }

        if !status.is_success() {
            let body_text = response.text().await.unwrap_or_default();
            if let Ok(body) = serde_json::from_str::<OpenAiResponse>(&body_text) {
                if let Some(error) = body.error {
                    anyhow::bail!(
                        "OpenAI-compatible API error ({}): {}",
                        status,
                        error.message()
                    );
                }
            }
            anyhow::bail!(
                "OpenAI-compatible API returned HTTP {}: {}",
                status,
                body_text
            );
        }

        let body: OpenAiResponse = response
            .json()
            .await
            .context("Failed to parse OpenAI-compatible response")?;

        let raw_text =
            extract_text(&body).ok_or_else(|| anyhow::anyhow!("No text in LLM response"))?;
        return Ok(sanitize_branch_name(&raw_text));
    }

    anyhow::bail!(
        "OpenAI-compatible API rate limited after {} retries. {}",
        MAX_RETRIES,
        last_error.unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_text_from_message_string() {
        let json = r#"{
            "choices": [{
                "message": { "role": "assistant", "content": "feat/add-oauth-auth" }
            }]
        }"#;
        let body: OpenAiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(extract_text(&body), Some("feat/add-oauth-auth".to_string()));
    }

    #[test]
    fn extract_text_from_message_parts() {
        let json = r#"{
            "choices": [{
                "message": {
                    "content": [
                        { "type": "text", "text": "fix/empty-file-crash" }
                    ]
                }
            }]
        }"#;
        let body: OpenAiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(extract_text(&body), Some("fix/empty-file-crash".to_string()));
    }

    #[test]
    fn extract_text_from_legacy_choice_text() {
        let json = r#"{
            "choices": [{
                "text": "docs/update-readme"
            }]
        }"#;
        let body: OpenAiResponse = serde_json::from_str(json).unwrap();
        assert_eq!(extract_text(&body), Some("docs/update-readme".to_string()));
    }
}
