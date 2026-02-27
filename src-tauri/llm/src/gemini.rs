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
    content: CandidateContent,
}

#[derive(Deserialize)]
struct CandidateContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize)]
struct ResponsePart {
    text: Option<String>,
}

#[derive(Deserialize)]
struct GeminiError {
    message: String,
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

        let body: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini response")?;

        if let Some(error) = body.error {
            anyhow::bail!("Gemini API error ({}): {}", status, error.message);
        }

        let raw_text = body
            .candidates
            .and_then(|c| c.into_iter().next())
            .and_then(|c| c.content.parts.into_iter().next())
            .and_then(|p| p.text)
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
}
