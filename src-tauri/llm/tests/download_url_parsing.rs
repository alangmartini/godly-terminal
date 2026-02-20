//! Bug #207: SmolLM2 download fails with "RelativeUrlWithoutBase" URL parsing error.
//!
//! hf-hub 0.3.2 constructs invalid (relative) URLs when calling
//! `api.model(repo).get(filename)`, causing the download to fail before any
//! network transfer occurs.
//!
//! These tests verify that:
//! 1. hf-hub can resolve HuggingFace repo file URLs without RelativeUrlWithoutBase errors
//! 2. `download_model()` succeeds end-to-end when given a valid temp directory

/// Bug #207: hf-hub must construct valid URLs for the tokenizer repo.
///
/// The tokenizer file is small (~2MB JSON) so this test is fast.
/// If hf-hub has a URL construction bug (RelativeUrlWithoutBase), it fails immediately
/// without any network transfer.
#[test]
fn hf_hub_resolves_tokenizer_url() {
    // Bug #207: hf-hub 0.3.2 fails with "RelativeUrlWithoutBase: relative URL without a base"
    let api = hf_hub::api::sync::Api::new().expect("Failed to create HF Hub API");
    let repo = api.model("HuggingFaceTB/SmolLM2-135M-Instruct".to_string());

    let result = repo.get("tokenizer.json");
    assert!(
        result.is_ok(),
        "hf-hub should resolve tokenizer.json URL but got: {}",
        result.unwrap_err()
    );

    // Verify the resolved path exists and is a real file
    let path = result.unwrap();
    assert!(path.exists(), "Downloaded tokenizer.json should exist at {:?}", path);
}

/// Bug #207: hf-hub must construct valid URLs for the GGUF model repo.
///
/// This test only checks URL resolution, not the full download.
/// We use repo.info() metadata if available, or attempt a HEAD-like check.
/// Since repo.get() downloads the file (~110MB), we skip the actual GGUF download
/// and focus on the tokenizer to prove URL construction works.
#[test]
fn hf_hub_resolves_gguf_repo_url() {
    // Bug #207: hf-hub 0.3.2 fails URL construction for all repos, not just tokenizer
    let api = hf_hub::api::sync::Api::new().expect("Failed to create HF Hub API");
    let repo = api.model("bartowski/SmolLM2-135M-Instruct-GGUF".to_string());

    // Just fetch a small metadata file to prove URL construction works.
    // README.md is tiny and present in all HuggingFace repos.
    let result = repo.get("README.md");
    assert!(
        result.is_ok(),
        "hf-hub should resolve GGUF repo URL but got: {}",
        result.unwrap_err()
    );
}

/// Bug #207: The full download_model() pipeline must succeed.
///
/// This exercises the exact code path that fails in production:
/// `download_model()` -> `Api::new()` -> `repo.get()` for both GGUF and tokenizer.
///
/// Uses a temp directory so downloads don't conflict with production files.
/// Downloads ~110MB on first run (cached by hf-hub afterward).
#[test]
fn download_model_succeeds() {
    // Bug #207: download_model fails with RelativeUrlWithoutBase before any transfer
    let temp = std::env::temp_dir().join("godly-llm-test-download-207");
    // Clean up from any previous failed run
    let _ = std::fs::remove_dir_all(&temp);

    let result = godly_llm::download_model(&temp, |_downloaded, _total| {});

    // Clean up
    let _ = std::fs::remove_dir_all(&temp);

    assert!(
        result.is_ok(),
        "download_model should complete successfully but got: {:#}",
        result.unwrap_err()
    );
}

/// Bug #207: After successful download, both model files must exist.
///
/// Verifies the ModelPaths point to real files after download completes.
#[test]
fn download_model_creates_both_files() {
    // Bug #207: download never completes because URL construction fails
    let temp = std::env::temp_dir().join("godly-llm-test-files-207");
    let _ = std::fs::remove_dir_all(&temp);

    let result = godly_llm::download_model(&temp, |_, _| {});

    match result {
        Ok(paths) => {
            assert!(
                paths.gguf_path.exists(),
                "GGUF model file should exist at {:?}",
                paths.gguf_path
            );
            assert!(
                paths.tokenizer_path.exists(),
                "Tokenizer file should exist at {:?}",
                paths.tokenizer_path
            );
            assert!(paths.is_downloaded(), "ModelPaths::is_downloaded() should return true");
        }
        Err(e) => {
            panic!(
                "download_model should succeed but failed: {:#}",
                e
            );
        }
    }

    // Clean up
    let _ = std::fs::remove_dir_all(&temp);
}
