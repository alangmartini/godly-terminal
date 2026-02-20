//! Regression tests for Bug #199: SmolLM2 download error messages must include
//! the full error chain so users see the actual root cause (HTTP error, network
//! error, filesystem error, etc.), not just "Failed to download tokenizer".
//!
//! The fix: commands/llm.rs uses `{:#}` (alternate Display) for anyhow errors,
//! which outputs the full chain: "Failed to download tokenizer: connection refused".

use anyhow::Context;
use std::path::Path;

/// Simulate the error chain that occurs when hf-hub download fails.
/// download.rs wraps the hf-hub error with `.context("Failed to download tokenizer")`.
/// The command handler formats it as `format!("Download failed: {:#}", e)`.
fn simulate_download_error_chain() -> anyhow::Error {
    let root_cause: Result<(), _> = Err(std::io::Error::new(
        std::io::ErrorKind::ConnectionRefused,
        "connection refused: huggingface.co:443",
    ));

    root_cause
        .context("Failed to download tokenizer")
        .unwrap_err()
}

/// Match the exact formatting used in commands/llm.rs:43 (fixed version with {:#})
fn format_error_like_command_handler(e: &anyhow::Error) -> String {
    format!("Download failed: {:#}", e)
}

#[test]
fn error_message_includes_root_cause() {
    // Bug #199 regression: error chain must include the underlying cause
    let err = simulate_download_error_chain();
    let msg = format_error_like_command_handler(&err);

    assert!(
        msg.contains("connection refused"),
        "Error message should include root cause but got: '{}'",
        msg
    );
}

#[test]
fn error_message_contains_actionable_info_beyond_boilerplate() {
    // Bug #199 regression: after stripping context wrappers, actual error info remains
    let err = simulate_download_error_chain();
    let msg = format_error_like_command_handler(&err);

    let after_prefix = msg
        .strip_prefix("Download failed: ")
        .unwrap_or(&msg);
    let stripped = after_prefix
        .replace("Failed to download tokenizer", "")
        .trim()
        .to_string();

    assert!(
        !stripped.is_empty(),
        "Error message contains no actionable information beyond boilerplate. Full msg: '{}'",
        msg
    );
}

#[test]
fn error_chain_includes_both_context_and_cause() {
    let err = simulate_download_error_chain();
    let msg = format_error_like_command_handler(&err);

    assert!(
        msg.contains("Failed to download tokenizer"),
        "Should include context: '{}'",
        msg
    );
    assert!(
        msg.contains("connection refused"),
        "Should include root cause: '{}'",
        msg
    );
}

#[test]
fn download_with_nonexistent_path_gives_informative_error() {
    // Bug #199 regression: filesystem errors must include OS-level details
    use godly_llm::download_model;

    let invalid_dir = if cfg!(windows) {
        Path::new("\\\\?\\Z:\\nonexistent\\deeply\\nested\\path\\that\\cannot\\exist")
    } else {
        Path::new("/proc/nonexistent/path/that/cannot/exist")
    };

    let result = download_model(invalid_dir, |_, _| {});
    assert!(result.is_err(), "Download to invalid path should fail");

    let err = result.unwrap_err();
    // Format with {:#} (the fixed handler format)
    let msg = format!("Download failed: {:#}", err);

    // Should include both the context AND the OS error
    assert!(
        msg.contains("model directory"),
        "Error should include context about model directory but got: '{}'",
        msg
    );
    // The OS error (e.g., "The system cannot find the path specified") should be present
    assert!(
        msg.len() > "Download failed: Failed to create model directory".len(),
        "Error should include OS-level details, not just context. Got: '{}'",
        msg
    );
}
