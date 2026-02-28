//! Reproduction tests for Issue #446:
//! Branch Name AI: "Failed to parse Gemini response" with gemini-3-flash-preview
//!
//! Bug: Using gemini-3-flash-preview as the model causes
//! `response.json::<GeminiResponse>()` to fail at gemini.rs:132-135.
//!
//! These tests reproduce the deserialization vulnerabilities that cause the
//! "Failed to parse Gemini response" error. The struct definitions below are
//! exact copies from `gemini.rs` to test identical deserialization behavior.

use serde::Deserialize;

// --- Struct definitions copied from gemini.rs (exact same layout) ---

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiError>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: Option<CandidateContent>,
}

#[derive(Deserialize, Debug)]
struct CandidateContent {
    parts: Vec<ResponsePart>,
}

#[derive(Deserialize, Debug)]
struct ResponsePart {
    text: Option<String>,
    thought: Option<bool>,
}

#[derive(Deserialize, Debug)]
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

// --- Helper to extract text (mirrors gemini.rs logic — skips thinking parts) ---

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

// === Baseline tests (verify known-good formats parse correctly) ===

#[test]
fn baseline_standard_gemini_2_response_parses() {
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

    let response: GeminiResponse = serde_json::from_str(json).expect("standard response should parse");
    assert_eq!(extract_text(&response), Some("feat/add-oauth-auth".to_string()));
}

#[test]
fn baseline_error_response_with_object_parses() {
    let json = r#"{
        "error": {
            "code": 429,
            "message": "Resource has been exhausted (e.g. check quota).",
            "status": "RESOURCE_EXHAUSTED"
        }
    }"#;

    let response: GeminiResponse = serde_json::from_str(json).expect("error response should parse");
    assert!(response.error.is_some());
    assert!(response.error.unwrap().message().contains("exhausted"));
}

#[test]
fn baseline_model_not_found_error_parses() {
    let json = r#"{
        "error": {
            "code": 404,
            "message": "models/nonexistent-model is not found for API version v1beta, or is not supported for generateContent.",
            "status": "NOT_FOUND"
        }
    }"#;

    let response: GeminiResponse = serde_json::from_str(json).expect("404 error should parse");
    assert!(response.error.is_some());
    assert!(response.error.unwrap().message().contains("not found"));
}

// === Bug reproduction: Gemini 3 thinking model responses ===

/// Bug #446: Gemini 3 Flash Preview returns thinking parts alongside text parts.
/// The response should parse correctly even with extra fields like `thought`.
#[test]
fn gemini_3_thinking_response_with_thought_parts_parses() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [
                    {"thought": true, "text": "Let me think about a good branch name for OAuth authentication..."},
                    {"text": "feat/add-oauth-auth"}
                ],
                "role": "model"
            },
            "finishReason": "STOP"
        }],
        "usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 10, "thoughtsTokenCount": 25, "totalTokenCount": 85},
        "modelVersion": "gemini-3-flash-preview"
    }"#;

    let response: GeminiResponse = serde_json::from_str(json)
        .expect("Gemini 3 thinking response should parse");
    // The first part is the thought, second is the actual text.
    // Current code takes the first part, which would be the thinking content.
    // This is a logic bug but not a parsing bug.
    assert!(response.candidates.is_some());
}

/// Bug #446: Gemini 3 responses include thoughtSignature fields in parts.
/// These should be ignored by serde deserialization.
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

    let response: GeminiResponse = serde_json::from_str(json)
        .expect("response with thoughtSignature should parse");
    assert!(extract_text(&response).is_some());
}

// === Bug reproduction: candidate without content (safety block) ===

/// Bug #446: When the API returns a candidate blocked by safety filters,
/// the candidate has NO `content` field — only `finishReason` and `safetyRatings`.
/// The current `Candidate` struct has `content: CandidateContent` (non-optional),
/// so deserialization FAILS with "missing field `content`".
///
/// Expected: Should parse gracefully (content should be Optional).
/// Actual: Deserialization fails → "Failed to parse Gemini response"
#[test]
fn candidate_without_content_safety_blocked_should_parse() {
    let json = r#"{
        "candidates": [{
            "finishReason": "SAFETY",
            "safetyRatings": [
                {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "probability": "HIGH"}
            ]
        }],
        "promptFeedback": {
            "safetyRatings": [
                {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "probability": "NEGLIGIBLE"}
            ]
        }
    }"#;

    // Bug #446: This SHOULD parse (candidates exist but content is blocked).
    // The code should handle this gracefully with a clear error like
    // "Response blocked by safety filters" instead of "Failed to parse Gemini response".
    let result: Result<GeminiResponse, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "Candidate without content should deserialize (content should be Optional). \
         Got error: {:?}",
        result.err()
    );
}

/// Bug #446: When the model hits MAX_TOKENS during thinking, it may return
/// a candidate with finishReason but no content (all tokens used by thinking).
#[test]
fn candidate_without_content_max_tokens_should_parse() {
    let json = r#"{
        "candidates": [{
            "finishReason": "MAX_TOKENS"
        }],
        "usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 0, "thoughtsTokenCount": 30, "totalTokenCount": 80},
        "modelVersion": "gemini-3-flash-preview"
    }"#;

    let result: Result<GeminiResponse, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "Candidate with MAX_TOKENS and no content should deserialize. \
         Got error: {:?}",
        result.err()
    );
}

// === Bug reproduction: error response as string instead of object ===

/// Bug #446: Some API error responses may return `error` as a plain string
/// instead of an object with a `message` field. The current `GeminiError` struct
/// expects an object, so this causes deserialization to fail.
#[test]
fn error_as_string_should_parse() {
    let json = r#"{
        "error": "Model gemini-3-flash-preview is not available."
    }"#;

    let result: Result<GeminiResponse, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "Error as string should deserialize (GeminiError should handle both string and object). \
         Got error: {:?}",
        result.err()
    );
}

// === Bug reproduction: non-JSON responses ===

/// Bug #446: If the API returns HTML (e.g., proxy error page), parsing fails
/// with an unhelpful "Failed to parse Gemini response" message.
/// The code should check HTTP status before parsing and provide a clear error.
///
/// Note: This test validates the deserialization behavior directly since we can't
/// mock the HTTP layer without modifying source code.
#[test]
fn non_json_response_fails_deserialization() {
    let html = r#"<html><body><h1>502 Bad Gateway</h1></body></html>"#;
    let result: Result<GeminiResponse, _> = serde_json::from_str(html);
    // This WILL fail (as expected for non-JSON). The real bug is that the code
    // doesn't check HTTP status before attempting JSON parsing, so the user gets
    // "Failed to parse Gemini response" instead of "API returned HTTP 502".
    assert!(result.is_err(), "HTML should not parse as GeminiResponse");
}

/// Bug #446: Empty response body.
#[test]
fn empty_response_fails_deserialization() {
    let result: Result<GeminiResponse, _> = serde_json::from_str("");
    assert!(result.is_err(), "Empty body should not parse as GeminiResponse");
}

// === Integration test: calls real Gemini API ===

/// Bug #446: Integration test that reproduces the exact user scenario.
/// Requires GEMINI_API_KEY env var to run.
///
/// Run with: GEMINI_API_KEY=your_key cargo nextest run -p godly-llm --test gemini_response_parsing
#[tokio::test]
async fn gemini_3_flash_preview_should_generate_branch_name() {
    let api_key = match std::env::var("GEMINI_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("GEMINI_API_KEY not set — skipping integration test");
            return;
        }
    };

    // Bug #446: This call fails with "Failed to parse Gemini response"
    // when using gemini-3-flash-preview.
    let result = godly_llm::generate_branch_name_gemini(
        &api_key,
        "Add user authentication with OAuth",
        "gemini-3-flash-preview",
    )
    .await;

    assert!(
        result.is_ok(),
        "gemini-3-flash-preview should produce a valid branch name, but got: {:?}",
        result.err()
    );

    let name = result.unwrap();
    assert!(!name.is_empty(), "branch name should not be empty");
    assert!(
        name.contains('/') || name.contains('-'),
        "branch name should be kebab-case with prefix, got: {}",
        name
    );
}

/// Baseline: gemini-2.5-flash should work (proves the API key is valid).
#[tokio::test]
async fn baseline_gemini_2_5_flash_generates_branch_name() {
    let api_key = match std::env::var("GEMINI_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => {
            eprintln!("GEMINI_API_KEY not set — skipping integration test");
            return;
        }
    };

    let result = godly_llm::generate_branch_name_gemini(
        &api_key,
        "Fix crash when opening empty file",
        "gemini-2.5-flash",
    )
    .await;

    assert!(
        result.is_ok(),
        "gemini-2.5-flash should work as baseline, but got: {:?}",
        result.err()
    );
}
