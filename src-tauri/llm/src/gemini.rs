use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::branch_name::sanitize_branch_name;

const GEMINI_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

const SYSTEM_PROMPT: &str = "\
You are a git branch name generator. Given a description of a task, output ONLY a short, \
kebab-case branch name. Rules:\n\
- Use lowercase letters, numbers, and hyphens only\n\
- Start with a conventional prefix: feat/, fix/, refactor/, docs/, chore/, test/\n\
- Keep it under 50 characters total\n\
- No explanations, just the branch name\n\
\n\
Examples:\n\
Input: \"Add user authentication with OAuth\"\n\
Output: feat/add-oauth-auth\n\
Input: \"Fix crash when opening empty file\"\n\
Output: fix/empty-file-crash\n\
Input: \"Refactor database connection pooling\"\n\
Output: refactor/db-connection-pool";

#[derive(Serialize)]
struct GeminiRequest {
    system_instruction: SystemInstruction,
    contents: Vec<Content>,
    generation_config: GenerationConfig,
}

#[derive(Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Serialize)]
struct Part {
    text: String,
}

#[derive(Serialize)]
struct GenerationConfig {
    temperature: f32,
    max_output_tokens: u32,
}

#[derive(Deserialize)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiError>,
}

#[derive(Deserialize)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: Option<String>,
    thought: Option<bool>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum GeminiError {
    Structured { message: String },
    Plain(String),
}

impl GeminiError {
    fn message(&self) -> &str {
        match self {
            GeminiError::Structured { message } => message,
            GeminiError::Plain(s) => s,
        }
    }
}

/// Maximum number of retries for rate-limited (429) requests.
const MAX_RETRIES: u32 = 3;

/// Generate a branch name using the Gemini API.
///
/// `model` is the Gemini model ID (e.g. "gemini-2.0-flash-lite", "gemini-2.0-flash").
/// Retries up to 3 times with exponential backoff on 429 rate-limit responses.
pub async fn generate_branch_name_gemini(
    api_key: &str,
    description: &str,
    model: &str,
) -> Result<String> {
    let client = reqwest::Client::new();

    let request = GeminiRequest {
        system_instruction: SystemInstruction {
            parts: vec![Part {
                text: SYSTEM_PROMPT.to_string(),
            }],
        },
        contents: vec![Content {
            parts: vec![Part {
                text: description.to_string(),
            }],
        }],
        generation_config: GenerationConfig {
            temperature: 0.3,
            max_output_tokens: 30,
        },
    };

    let url = format!("{}/{}:generateContent?key={}", GEMINI_API_BASE, model, api_key);

    let mut last_error = None;
    for attempt in 0..=MAX_RETRIES {
        if attempt > 0 {
            let delay = std::time::Duration::from_millis(500 * 2u64.pow(attempt - 1));
            tokio::time::sleep(delay).await;
        }

        let response = client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to call Gemini API")?;

        let status = response.status();

        if status.as_u16() == 429 {
            last_error = Some(format!("Rate limited (429) on attempt {}", attempt + 1));
            continue;
        }

        if !status.is_success() {
            let body_text = response
                .text()
                .await
                .unwrap_or_default();
            if let Ok(body) = serde_json::from_str::<GeminiResponse>(&body_text) {
                if let Some(error) = body.error {
                    anyhow::bail!("Gemini API error ({}): {}", status, error.message());
                }
            }
            anyhow::bail!("Gemini API returned HTTP {}: {}", status, body_text);
        }

        let body: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini response")?;

        let raw_text = body
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|candidate| {
                candidate.content.and_then(|content| {
                    content
                        .parts
                        .into_iter()
                        .find(|p| p.thought != Some(true))
                        .and_then(|p| p.text)
                })
            })
            .ok_or_else(|| anyhow::anyhow!("No text in Gemini response"))?;

        return Ok(sanitize_branch_name(&raw_text));
    }

    anyhow::bail!(
        "Gemini API rate limited after {} retries. {}",
        MAX_RETRIES,
        last_error.unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn extract_text(body: &GeminiResponse) -> Option<String> {
        body.candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .and_then(|content| {
                content
                    .parts
                    .iter()
                    .find(|p| p.thought != Some(true))
                    .and_then(|p| p.text.clone())
            })
    }

    #[test]
    fn test_system_prompt_not_empty() {
        assert!(!SYSTEM_PROMPT.is_empty());
    }

    #[test]
    fn test_request_serialization() {
        let request = GeminiRequest {
            system_instruction: SystemInstruction {
                parts: vec![Part {
                    text: "system".to_string(),
                }],
            },
            contents: vec![Content {
                parts: vec![Part {
                    text: "test".to_string(),
                }],
            }],
            generation_config: GenerationConfig {
                temperature: 0.3,
                max_output_tokens: 30,
            },
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("system_instruction"));
        assert!(json.contains("generation_config"));
    }

    #[test]
    fn standard_gemini_2_response_parses() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [{"text": "feat/add-oauth-auth"}],
                    "role": "model"
                },
                "finishReason": "STOP",
                "safetyRatings": [{"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "probability": "NEGLIGIBLE"}]
            }],
            "usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 10, "totalTokenCount": 60}
        }"#;

        let response: GeminiResponse =
            serde_json::from_str(json).expect("standard response should parse");
        assert_eq!(
            extract_text(&response),
            Some("feat/add-oauth-auth".to_string())
        );
    }

    #[test]
    fn error_response_with_object_parses() {
        let json = r#"{
            "error": {
                "code": 429,
                "message": "Resource has been exhausted (e.g. check quota).",
                "status": "RESOURCE_EXHAUSTED"
            }
        }"#;

        let response: GeminiResponse =
            serde_json::from_str(json).expect("error response should parse");
        assert!(response.error.is_some());
        assert!(response.error.unwrap().message().contains("exhausted"));
    }

    #[test]
    fn model_not_found_error_parses() {
        let json = r#"{
            "error": {
                "code": 404,
                "message": "models/nonexistent-model is not found for API version v1beta.",
                "status": "NOT_FOUND"
            }
        }"#;

        let response: GeminiResponse =
            serde_json::from_str(json).expect("404 error should parse");
        assert!(response.error.unwrap().message().contains("not found"));
    }

    /// Bug #446: Gemini 3 returns thinking parts alongside text parts.
    /// extract_text must skip thought parts and return the real text.
    #[test]
    fn gemini_3_thinking_response_skips_thought_parts() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [
                        {"thought": true, "text": "Let me think about a good branch name..."},
                        {"text": "feat/add-oauth-auth"}
                    ],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "modelVersion": "gemini-3-flash-preview"
        }"#;

        let response: GeminiResponse =
            serde_json::from_str(json).expect("thinking response should parse");
        assert_eq!(
            extract_text(&response),
            Some("feat/add-oauth-auth".to_string())
        );
    }

    /// Bug #446: thoughtSignature fields should be ignored by serde.
    #[test]
    fn gemini_3_response_with_thought_signatures_parses() {
        let json = r#"{
            "candidates": [{
                "content": {
                    "parts": [
                        {"thought": true, "text": "analyzing..."},
                        {"text": "feat/add-oauth-auth", "thoughtSignature": "c2lnbmF0dXJlX2RhdGE="}
                    ],
                    "role": "model"
                },
                "finishReason": "STOP"
            }],
            "modelVersion": "gemini-3-flash-preview"
        }"#;

        let response: GeminiResponse =
            serde_json::from_str(json).expect("response with thoughtSignature should parse");
        assert_eq!(
            extract_text(&response),
            Some("feat/add-oauth-auth".to_string())
        );
    }

    /// Bug #446: Safety-blocked candidates have no `content` field.
    #[test]
    fn candidate_without_content_safety_blocked_parses() {
        let json = r#"{
            "candidates": [{
                "finishReason": "SAFETY",
                "safetyRatings": [
                    {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "probability": "HIGH"}
                ]
            }]
        }"#;

        let response: GeminiResponse =
            serde_json::from_str(json).expect("safety-blocked candidate should parse");
        assert_eq!(extract_text(&response), None);
    }

    /// Bug #446: MAX_TOKENS candidates may have no content (all tokens used by thinking).
    #[test]
    fn candidate_without_content_max_tokens_parses() {
        let json = r#"{
            "candidates": [{
                "finishReason": "MAX_TOKENS"
            }],
            "modelVersion": "gemini-3-flash-preview"
        }"#;

        let response: GeminiResponse =
            serde_json::from_str(json).expect("MAX_TOKENS candidate should parse");
        assert_eq!(extract_text(&response), None);
    }

    /// Bug #446: Error as plain string instead of object.
    #[test]
    fn error_as_string_parses() {
        let json = r#"{
            "error": "Model gemini-3-flash-preview is not available."
        }"#;

        let response: GeminiResponse =
            serde_json::from_str(json).expect("string error should parse");
        assert_eq!(
            response.error.unwrap().message(),
            "Model gemini-3-flash-preview is not available."
        );
    }

    #[test]
    fn non_json_response_fails_deserialization() {
        let html = r#"<html><body><h1>502 Bad Gateway</h1></body></html>"#;
        assert!(serde_json::from_str::<GeminiResponse>(html).is_err());
    }

    #[test]
    fn empty_response_fails_deserialization() {
        assert!(serde_json::from_str::<GeminiResponse>("").is_err());
    }
}
