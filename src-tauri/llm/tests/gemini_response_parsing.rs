//! Integration tests for Gemini API branch name generation.
//! Requires GEMINI_API_KEY env var — skipped otherwise.
//!
//! Deserialization unit tests live inline in `src/gemini.rs`.

fn require_api_key() -> Option<String> {
    match std::env::var("GEMINI_API_KEY") {
        Ok(key) if !key.is_empty() => Some(key),
        _ => {
            eprintln!("GEMINI_API_KEY not set — skipping integration test");
            None
        }
    }
}

/// Bug #446: gemini-3-flash-preview returns thinking parts + optional content.
#[tokio::test]
async fn gemini_3_flash_preview_should_generate_branch_name() {
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
    let Some(api_key) = require_api_key() else {
        return;
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
