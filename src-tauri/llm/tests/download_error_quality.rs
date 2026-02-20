//! Bug #199: SmolLM2 download error messages drop the root cause.
//!
//! The command handler in commands/llm.rs formats anyhow errors with `{}` (Display)
//! which only shows the outermost context, losing the actual HTTP/network error.
//! Should use `{:#}` to preserve the full error chain.

use anyhow::Context;
use std::path::Path;

/// Simulate the error chain that occurs when hf-hub download fails.
/// The download.rs code wraps the hf-hub error with `.context("Failed to download tokenizer")`.
/// The command handler then formats it as `format!("Download failed: {}", e)`.
fn simulate_download_error_chain() -> anyhow::Error {
    // Simulate the low-level error from hf-hub (e.g., HTTP 404, connection refused, etc.)
    let root_cause: Result<(), _> = Err(std::io::Error::new(
        std::io::ErrorKind::ConnectionRefused,
        "connection refused: huggingface.co:443",
    ));

    // This is what download.rs does: wraps with .context()
    let download_result: anyhow::Result<()> = root_cause
        .context("Failed to download tokenizer");

    download_result.unwrap_err()
}

/// Reproduce the exact error formatting used in commands/llm.rs:43
fn format_error_like_command_handler(e: &anyhow::Error) -> String {
    // Bug #199: This is how the command handler formats the error
    format!("Download failed: {}", e)
}

/// The correct way to format anyhow errors (preserves chain)
fn format_error_with_chain(e: &anyhow::Error) -> String {
    format!("Download failed: {:#}", e)
}

#[test]
fn error_message_should_include_root_cause() {
    // Bug #199: When download fails, the error message shown to the user
    // should include the actual root cause, not just "Failed to download tokenizer"
    let err = simulate_download_error_chain();

    let msg = format_error_like_command_handler(&err);

    // The message should contain the root cause (e.g., "connection refused")
    // This FAILS because `{}` only shows the outermost context
    assert!(
        msg.contains("connection refused"),
        "Error message should include root cause but got: '{}'",
        msg
    );
}

#[test]
fn error_message_should_not_just_say_failed_to_download() {
    // Bug #199: The current error "Download failed: Failed to download tokenizer"
    // is useless - it doesn't tell the user WHY it failed
    let err = simulate_download_error_chain();

    let msg = format_error_like_command_handler(&err);

    // The message should contain MORE than just the context wrapper
    // Strip the prefix and the known context to see if anything useful remains
    let after_prefix = msg
        .strip_prefix("Download failed: ")
        .unwrap_or(&msg);
    let stripped = after_prefix
        .replace("Failed to download tokenizer", "")
        .trim()
        .to_string();

    // After removing the boilerplate, there should be actual error info left
    // This FAILS because {} only gives "Failed to download tokenizer" with no root cause
    assert!(
        !stripped.is_empty(),
        "Error message contains no actionable information beyond boilerplate. Full msg: '{}'",
        msg
    );
}

#[test]
fn correct_format_preserves_error_chain() {
    // Verify that {:#} (alternate Display) DOES preserve the chain
    // This is the reference for what the fix should produce
    let err = simulate_download_error_chain();

    let msg = format_error_with_chain(&err);

    assert!(
        msg.contains("connection refused"),
        "Alternate display should include root cause: '{}'",
        msg
    );
    assert!(
        msg.contains("Failed to download tokenizer"),
        "Alternate display should include context: '{}'",
        msg
    );
}

#[test]
fn download_with_nonexistent_path_gives_informative_error() {
    // Bug #199: Even filesystem errors should include details
    // Try downloading to a path that will fail at directory creation
    use godly_llm::download_model;

    // Use an invalid path that will fail
    let invalid_dir = if cfg!(windows) {
        Path::new("\\\\?\\Z:\\nonexistent\\deeply\\nested\\path\\that\\cannot\\exist")
    } else {
        Path::new("/proc/nonexistent/path/that/cannot/exist")
    };

    let result = download_model(invalid_dir, |_, _| {});
    assert!(result.is_err(), "Download to invalid path should fail");

    let err = result.unwrap_err();
    // Format the error the way the command handler does (Bug #199 pattern)
    let msg = format!("Download failed: {}", err);

    // The error should include the path or filesystem error details
    // With `{}`, it just says "Failed to create model directory" and drops the OS error
    assert!(
        msg.contains("model directory") || msg.contains("nonexistent") || msg.contains("denied") || msg.contains("No such file"),
        "Error should include path or OS details but got: '{}'",
        msg
    );

    // But more importantly, the OS-level error should be visible
    let msg_chain = format!("Download failed: {:#}", err);
    // The chain format should have MORE information
    assert!(
        msg_chain.len() > msg.len(),
        "Chain format ({{:#}}) should have more detail than display ({{{{}}}}). Display: '{}', Chain: '{}'",
        msg, msg_chain
    );
}
