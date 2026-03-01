//! Regression tests for issue #446: "Failed to parse Gemini response" with gemini-3-flash-preview.
//!
//! The original #446 fix handled optional content, string errors, and thinking parts.
//! These tests verify those fixes still work AND test new response formats that
//! the Gemini API may return for gemini-3-flash-preview.
//!
//! Integration tests (real API) require GEMINI_API_KEY env var — skipped otherwise.

use serde::Deserialize;

// --- Mirror of private GeminiResponse structs for raw response testing ---
// These must match the structs in src/gemini.rs exactly.

#[derive(Deserialize, Debug)]
struct GeminiResponse {
    candidates: Option<Vec<Candidate>>,
    error: Option<GeminiError>,
}

#[derive(Deserialize, Debug)]
struct Candidate {
    content: Option<CandidateContent>,
    #[serde(rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
struct CandidateContent {
    #[serde(default)]
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

fn extract_text(body: &GeminiResponse) -> Option<String> {
    body.candidates
        .as_ref()
        .and_then(|c| c.first())
        .and_then(|c| c.content.as_ref())
        .and_then(|content| {
            content
                .parts
                .iter()
                .find_map(|p| {
                    if p.thought != Some(true) { p.text.clone() } else { None }
                })
        })
}

fn require_api_key() -> Option<String> {
    match std::env::var("GEMINI_API_KEY") {
        Ok(key) if !key.is_empty() => Some(key),
        _ => {
            eprintln!("GEMINI_API_KEY not set — skipping integration test");
            None
        }
    }
}

// =============================================================================
// Integration tests: hit the real Gemini API
// =============================================================================

/// Bug #446 regression: Call gemini-3-flash-preview via the real API and capture
/// the raw response body. If deserialization fails, the raw body is printed so
/// we can see exactly what changed.
#[tokio::test]
async fn raw_gemini_3_flash_preview_response_deserializes() {
    let Some(api_key) = require_api_key() else {
        return;
    };

    let client = reqwest::Client::new();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent?key={}",
        api_key
    );

    let request_body = serde_json::json!({
        "system_instruction": {
            "parts": [{"text": "You are a git branch name generator. Given a description, output ONLY a short kebab-case branch name with a prefix like feat/, fix/, etc."}]
        },
        "contents": [{
            "parts": [{"text": "Add user authentication with OAuth"}]
        }],
        "generation_config": {
            "temperature": 0.3,
            "max_output_tokens": 30
        }
    });

    let response = client
        .post(&url)
        .json(&request_body)
        .send()
        .await
        .expect("HTTP request should succeed");

    let status = response.status();
    let raw_body = response
        .text()
        .await
        .expect("Should be able to read response body");

    // Print raw body for debugging regardless of outcome
    eprintln!("=== Raw Gemini 3 Flash Preview Response ===");
    eprintln!("Status: {}", status);
    eprintln!("Body: {}", &raw_body[..raw_body.len().min(2000)]);
    eprintln!("=== End Response ===");

    assert!(
        status.is_success(),
        "API should return 200, got {}. Body: {}",
        status,
        &raw_body[..raw_body.len().min(500)]
    );

    // Try to parse with our struct — this is where #446 regression would show
    let parsed: Result<GeminiResponse, _> = serde_json::from_str(&raw_body);
    assert!(
        parsed.is_ok(),
        "Bug #446 regression: Failed to parse Gemini response.\n\
         Deserialization error: {:?}\n\
         Raw body (first 1000 chars): {}",
        parsed.err(),
        &raw_body[..raw_body.len().min(1000)]
    );

    let body = parsed.unwrap();

    // Verify we can extract text (not just parse the envelope)
    let text = extract_text(&body);
    assert!(
        text.is_some(),
        "Bug #446 regression: Parsed response but no text found.\n\
         Candidates: {:?}\n\
         Raw body (first 1000 chars): {}",
        body.candidates,
        &raw_body[..raw_body.len().min(1000)]
    );
}

/// Bug #446 regression: The public generate_branch_name_gemini function should
/// succeed with gemini-3-flash-preview.
#[tokio::test]
async fn generate_branch_name_gemini_3_flash_preview_succeeds() {
    let Some(api_key) = require_api_key() else {
        return;
    };

    let result = godly_llm::generate_branch_name_gemini(
        &api_key,
        "Add user authentication with OAuth",
        "gemini-3-flash-preview",
    )
    .await;

    assert!(
        result.is_ok(),
        "Bug #446 regression: generate_branch_name_gemini failed with gemini-3-flash-preview: {:?}",
        result.err()
    );

    let name = result.unwrap();
    assert!(!name.is_empty(), "Branch name should not be empty");
    assert!(
        name.contains('/') || name.contains('-'),
        "Branch name should be kebab-case, got: {}",
        name
    );
}

/// Baseline: gemini-2.0-flash should work, proving the API key is valid.
#[tokio::test]
async fn baseline_gemini_2_0_flash_generates_branch_name() {
    let Some(api_key) = require_api_key() else {
        return;
    };

    let result = godly_llm::generate_branch_name_gemini(
        &api_key,
        "Fix crash when opening empty file",
        "gemini-2.0-flash",
    )
    .await;

    assert!(
        result.is_ok(),
        "Baseline gemini-2.0-flash should work, but got: {:?}",
        result.err()
    );
}

// =============================================================================
// Unit tests: mock responses that test current and future API formats
// =============================================================================

/// Bug #446 regression (ROOT CAUSE): Gemini 3 returns `"content": {}` (empty object)
/// when all tokens are consumed by thinking. The `CandidateContent` struct requires
/// `parts: Vec<ResponsePart>`, so serde fails with "missing field `parts`".
///
/// Real API response captured 2026-02-28:
/// ```json
/// {
///   "candidates": [{
///     "content": {},
///     "finishReason": "MAX_TOKENS",
///     "index": 0
///   }],
///   "usageMetadata": {
///     "promptTokenCount": 36,
///     "totalTokenCount": 63,
///     "thoughtsTokenCount": 27
///   },
///   "modelVersion": "gemini-3-flash-preview"
/// }
/// ```
///
/// With max_output_tokens=30, the model spends 27 tokens thinking and produces
/// no text output. The API returns `content: {}` instead of omitting it.
#[test]
fn empty_content_object_from_thinking_model_should_parse() {
    // Exact response from gemini-3-flash-preview API (2026-02-28)
    let json = r#"{
        "candidates": [{
            "content": {},
            "finishReason": "MAX_TOKENS",
            "index": 0
        }],
        "usageMetadata": {
            "promptTokenCount": 36,
            "totalTokenCount": 63,
            "promptTokensDetails": [{"modality": "TEXT", "tokenCount": 36}],
            "thoughtsTokenCount": 27
        },
        "modelVersion": "gemini-3-flash-preview",
        "responseId": "no2jae_LM4yiqtsP8MDj-AM"
    }"#;

    // Bug #446: This fails with "missing field `parts`" because CandidateContent
    // has `parts: Vec<ResponsePart>` (required), but the API returns `content: {}`
    // (empty object with no parts field).
    let result: Result<GeminiResponse, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "Empty content object should parse. Error: {:?}",
        result.err()
    );

    let response = result.unwrap();
    // No text expected — all tokens used by thinking
    assert_eq!(extract_text(&response), None);
}

/// Bug #446 regression: Same empty content but with `"content": {"role": "model"}`
/// (no parts field, but role is present). Another variant of the same issue.
#[test]
fn content_with_role_but_no_parts_should_parse() {
    let json = r#"{
        "candidates": [{
            "content": {"role": "model"},
            "finishReason": "MAX_TOKENS"
        }],
        "modelVersion": "gemini-3-flash-preview"
    }"#;

    let result: Result<GeminiResponse, _> = serde_json::from_str(json);
    assert!(
        result.is_ok(),
        "Content with role but no parts should parse. Error: {:?}",
        result.err()
    );
}

/// Bug #446 regression: Content with empty parts array should parse fine.
#[test]
fn content_with_empty_parts_array_should_parse() {
    let json = r#"{
        "candidates": [{
            "content": {"parts": [], "role": "model"},
            "finishReason": "MAX_TOKENS"
        }],
        "modelVersion": "gemini-3-flash-preview"
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("content with empty parts should parse");
    assert_eq!(extract_text(&response), None);
}

/// Bug #446: Gemini 3 thinking response with thoughtSignature on the text part.
/// The thoughtSignature is a base64-encoded field that should be ignored.
#[test]
fn gemini_3_thinking_with_thought_signature_on_text_part() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [
                    {"thought": true, "text": "Let me think about a good branch name..."},
                    {"text": "feat/add-oauth-auth", "thoughtSignature": "c2lnbmF0dXJlX2RhdGE="}
                ],
                "role": "model"
            },
            "finishReason": "STOP"
        }],
        "modelVersion": "gemini-3-flash-preview",
        "usageMetadata": {
            "promptTokenCount": 50,
            "candidatesTokenCount": 10,
            "thoughtsTokenCount": 100,
            "totalTokenCount": 160
        }
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("thinking response with thoughtSignature should parse");
    assert_eq!(
        extract_text(&response),
        Some("feat/add-oauth-auth".to_string())
    );
}

/// Bug #446 regression: Gemini 3 might return only thought parts with no text part.
/// This happens when all tokens are used by thinking (MAX_TOKENS + thinking model).
#[test]
fn gemini_3_all_thinking_no_text_part() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [
                    {"thought": true, "text": "Hmm, let me think about this..."},
                    {"thought": true, "text": "A good branch name would be..."}
                ],
                "role": "model"
            },
            "finishReason": "MAX_TOKENS"
        }],
        "modelVersion": "gemini-3-flash-preview"
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("all-thinking response should parse");
    // No non-thought text → extract returns None
    assert_eq!(extract_text(&response), None);
}

/// Bug #446 regression: Response with groundingMetadata (search grounding).
/// New field that didn't exist when the original fix was written.
#[test]
fn gemini_3_response_with_grounding_metadata() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [{"text": "feat/add-oauth-auth"}],
                "role": "model"
            },
            "finishReason": "STOP",
            "groundingMetadata": {
                "searchEntryPoint": {"renderedContent": "<div>...</div>"},
                "groundingChunks": [{"web": {"uri": "https://example.com", "title": "Example"}}],
                "groundingSupports": [],
                "webSearchQueries": ["oauth authentication"]
            }
        }],
        "usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 10, "totalTokenCount": 60}
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("response with grounding metadata should parse");
    assert_eq!(
        extract_text(&response),
        Some("feat/add-oauth-auth".to_string())
    );
}

/// Bug #446 regression: Response with urlContextMetadata.
#[test]
fn gemini_3_response_with_url_context_metadata() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [{"text": "feat/add-oauth"}],
                "role": "model"
            },
            "finishReason": "STOP",
            "urlContextMetadata": {
                "urlMetadata": [{"retrievedUrl": "https://example.com", "urlRetrievalStatus": "URL_RETRIEVAL_STATUS_SUCCESS"}]
            }
        }],
        "usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 5, "totalTokenCount": 55}
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("response with URL context metadata should parse");
    assert_eq!(
        extract_text(&response),
        Some("feat/add-oauth".to_string())
    );
}

/// Bug #446 regression: Response where parts contain functionCall objects
/// alongside text parts. Deserialization should succeed (unknown fields ignored).
/// find_map() skips parts without text and finds the actual text part.
#[test]
fn gemini_3_response_with_function_call_part_parses() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [
                    {"functionCall": {"name": "get_context", "args": {"query": "auth"}}},
                    {"text": "feat/add-oauth-auth"}
                ],
                "role": "model"
            },
            "finishReason": "STOP"
        }]
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("response with functionCall parts should parse");
    assert_eq!(
        extract_text(&response),
        Some("feat/add-oauth-auth".to_string()),
    );
}

/// Bug #446 regression: Response with executableCode part (code execution).
/// find_map() skips parts without text and finds the actual text part.
#[test]
fn gemini_3_response_with_executable_code_part_parses() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [
                    {"executableCode": {"language": "PYTHON", "code": "print('hello')"}},
                    {"codeExecutionResult": {"output": "hello", "outcome": "OUTCOME_OK"}},
                    {"text": "fix/code-execution-bug"}
                ],
                "role": "model"
            },
            "finishReason": "STOP"
        }]
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("response with executable code parts should parse");
    assert_eq!(
        extract_text(&response),
        Some("fix/code-execution-bug".to_string()),
    );
}

/// Bug #446 regression: Candidate with citationMetadata.
#[test]
fn gemini_3_candidate_with_citation_metadata() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [{"text": "feat/add-auth"}],
                "role": "model"
            },
            "finishReason": "STOP",
            "citationMetadata": {
                "citationSources": [{"startIndex": 0, "endIndex": 10, "uri": "https://example.com"}]
            }
        }]
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("response with citation metadata should parse");
    assert_eq!(
        extract_text(&response),
        Some("feat/add-auth".to_string())
    );
}

/// Bug #446 regression: Candidate with avgLogprobs field (newer Gemini response format).
#[test]
fn gemini_3_candidate_with_logprobs() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [{"text": "feat/add-login"}],
                "role": "model"
            },
            "finishReason": "STOP",
            "avgLogprobs": -0.123456
        }],
        "modelVersion": "gemini-3-flash-preview"
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("response with avgLogprobs should parse");
    assert_eq!(
        extract_text(&response),
        Some("feat/add-login".to_string())
    );
}

/// Bug #446 regression: Response with promptFeedback blocking the request.
/// This is a top-level field, not inside candidates.
#[test]
fn gemini_3_response_with_prompt_feedback_block() {
    let json = r#"{
        "promptFeedback": {
            "blockReason": "SAFETY",
            "safetyRatings": [
                {"category": "HARM_CATEGORY_SEXUALLY_EXPLICIT", "probability": "HIGH"}
            ]
        }
    }"#;

    let response: GeminiResponse = serde_json::from_str(json)
        .expect("response with only promptFeedback should parse (candidates absent)");
    assert!(response.candidates.is_none());
    assert_eq!(extract_text(&response), None);
}

/// Bug #446 regression: Empty candidates array (not null, but []).
#[test]
fn gemini_3_empty_candidates_array() {
    let json = r#"{
        "candidates": [],
        "usageMetadata": {"promptTokenCount": 50, "candidatesTokenCount": 0, "totalTokenCount": 50}
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("empty candidates array should parse");
    assert_eq!(extract_text(&response), None);
}

/// Bug #446 regression: Response with inlineData part (image generation).
/// The inlineData field is an object, not a string — should be silently ignored.
#[test]
fn gemini_3_response_with_inline_data_part() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [
                    {"text": "feat/add-image-gen"},
                    {"inlineData": {"mimeType": "image/png", "data": "iVBORw0KGgo="}}
                ],
                "role": "model"
            },
            "finishReason": "STOP"
        }]
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("response with inlineData parts should parse");
    assert_eq!(
        extract_text(&response),
        Some("feat/add-image-gen".to_string())
    );
}

/// Bug #446 regression: Response where the only non-thought part has no text field.
/// This can happen with functionCall-only or inlineData-only responses.
#[test]
fn gemini_3_no_text_in_any_non_thought_part() {
    let json = r#"{
        "candidates": [{
            "content": {
                "parts": [
                    {"thought": true, "text": "thinking..."},
                    {"inlineData": {"mimeType": "image/png", "data": "iVBORw0KGgo="}}
                ],
                "role": "model"
            },
            "finishReason": "STOP"
        }]
    }"#;

    let response: GeminiResponse =
        serde_json::from_str(json).expect("response with no text in non-thought parts should parse");
    // The inlineData part has text: None, so extract returns None
    assert_eq!(extract_text(&response), None);
}
