use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const REPO_GGUF: &str = "bartowski/SmolLM2-135M-Instruct-GGUF";
const FILENAME_GGUF: &str = "SmolLM2-135M-Instruct-Q4_K_M.gguf";
const REPO_TOKENIZER: &str = "HuggingFaceTB/SmolLM2-135M-Instruct";
const FILENAME_TOKENIZER: &str = "tokenizer.json";

/// Paths to model files in the app data directory.
#[derive(Debug, Clone)]
pub struct ModelPaths {
    pub model_dir: PathBuf,
    pub gguf_path: PathBuf,
    pub tokenizer_path: PathBuf,
}

impl ModelPaths {
    pub fn new(app_data_dir: &Path) -> Self {
        let model_dir = app_data_dir.join("models").join("smollm2-135m");
        let gguf_path = model_dir.join(FILENAME_GGUF);
        let tokenizer_path = model_dir.join(FILENAME_TOKENIZER);
        Self {
            model_dir,
            gguf_path,
            tokenizer_path,
        }
    }

    /// Check if both model files exist.
    pub fn is_downloaded(&self) -> bool {
        self.gguf_path.exists() && self.tokenizer_path.exists()
    }
}

/// Download the SmolLM2-135M GGUF model and tokenizer from HuggingFace.
/// Calls `on_progress(bytes_downloaded, total_bytes)` during download.
pub fn download_model<F>(app_data_dir: &Path, on_progress: F) -> Result<ModelPaths>
where
    F: Fn(u64, u64),
{
    let paths = ModelPaths::new(app_data_dir);
    std::fs::create_dir_all(&paths.model_dir)
        .with_context(|| format!("Failed to create model directory: {:?}", paths.model_dir))?;

    let api =
        hf_hub::api::sync::Api::new().context("Failed to initialize HuggingFace Hub API")?;

    // Download GGUF model file
    if !paths.gguf_path.exists() {
        eprintln!(
            "[llm] Downloading GGUF model from {}/{}",
            REPO_GGUF, FILENAME_GGUF
        );
        let repo = api.model(REPO_GGUF.to_string());
        let downloaded = repo.get(FILENAME_GGUF).context("Failed to download GGUF model")?;
        std::fs::copy(&downloaded, &paths.gguf_path)
            .context("Failed to copy GGUF to model directory")?;
        if let Ok(meta) = std::fs::metadata(&paths.gguf_path) {
            on_progress(meta.len(), meta.len());
        }
    }

    // Download tokenizer
    if !paths.tokenizer_path.exists() {
        eprintln!(
            "[llm] Downloading tokenizer from {}/{}",
            REPO_TOKENIZER, FILENAME_TOKENIZER
        );
        let repo = api.model(REPO_TOKENIZER.to_string());
        let downloaded = repo
            .get(FILENAME_TOKENIZER)
            .context("Failed to download tokenizer")?;
        std::fs::copy(&downloaded, &paths.tokenizer_path)
            .context("Failed to copy tokenizer to model directory")?;
    }

    eprintln!("[llm] Model download complete");
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_paths_construction() {
        let paths = ModelPaths::new(Path::new("/tmp/testapp"));
        assert!(paths.model_dir.ends_with("models/smollm2-135m"));
        assert!(paths
            .gguf_path
            .to_string_lossy()
            .contains(FILENAME_GGUF));
        assert!(paths
            .tokenizer_path
            .to_string_lossy()
            .contains(FILENAME_TOKENIZER));
    }

    #[test]
    fn test_not_downloaded_when_missing() {
        let paths = ModelPaths::new(Path::new("/nonexistent/path"));
        assert!(!paths.is_downloaded());
    }
}
