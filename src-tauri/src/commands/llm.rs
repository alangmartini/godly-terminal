use std::sync::Arc;

use tauri::{Emitter, State};

use crate::llm_state::LlmState;
use godly_llm::{
    download_model, download_model_custom, generate_branch_name, LlmEngine, LlmStatus, ModelPaths,
};

#[tauri::command]
pub async fn llm_get_status(llm: State<'_, Arc<LlmState>>) -> Result<LlmStatus, String> {
    Ok(llm.status.read().clone())
}

#[tauri::command]
pub async fn llm_download_model(
    app_handle: tauri::AppHandle,
    llm: State<'_, Arc<LlmState>>,
    hf_repo: Option<String>,
    hf_filename: Option<String>,
    tokenizer_repo: Option<String>,
    subdir: Option<String>,
) -> Result<(), String> {
    let app_data_dir = llm
        .get_app_data_dir()
        .ok_or_else(|| "App data directory not initialized".to_string())?;

    *llm.status.write() = LlmStatus::Downloading { progress: 0.0 };

    let handle = app_handle.clone();
    let status_ref = Arc::clone(&llm);

    tokio::task::spawn_blocking(move || {
        let progress_cb = |downloaded: u64, total: u64| {
            let progress = if total > 0 {
                (downloaded as f32) / (total as f32)
            } else {
                0.0
            };
            let _ = handle.emit("llm-download-progress", progress);
        };

        let result = match (hf_repo, hf_filename, tokenizer_repo, subdir) {
            (Some(repo), Some(filename), Some(tok_repo), Some(sub)) => {
                download_model_custom(&app_data_dir, &repo, &filename, &tok_repo, &sub, progress_cb)
                    .map(|_| ())
            }
            _ => download_model(&app_data_dir, progress_cb).map(|_| ()),
        };

        match result {
            Ok(_) => {
                *status_ref.status.write() = LlmStatus::Downloaded;
                Ok(())
            }
            Err(e) => {
                let msg = format!("Download failed: {:#}", e);
                *status_ref.status.write() = LlmStatus::Error(msg.clone());
                Err(msg)
            }
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn llm_load_model(
    llm: State<'_, Arc<LlmState>>,
    gguf_path: Option<String>,
    tokenizer_path: Option<String>,
    subdir: Option<String>,
    gguf_filename: Option<String>,
) -> Result<(), String> {
    let app_data_dir = llm
        .get_app_data_dir()
        .ok_or_else(|| "App data directory not initialized".to_string())?;

    *llm.status.write() = LlmStatus::Loading;

    let status_ref = Arc::clone(&llm);

    tokio::task::spawn_blocking(move || {
        let paths = match (gguf_path, tokenizer_path) {
            // Explicit file paths (custom GGUF picker)
            (Some(gguf), Some(tok)) => {
                ModelPaths::from_paths(gguf.into(), tok.into())
            }
            _ => {
                // Preset subdir or default
                match (subdir, gguf_filename) {
                    (Some(sub), Some(filename)) => ModelPaths::custom(&app_data_dir, &sub, &filename),
                    _ => ModelPaths::new(&app_data_dir),
                }
            }
        };

        match LlmEngine::load(&paths) {
            Ok(engine) => {
                *status_ref.engine.write() = Some(engine);
                *status_ref.status.write() = LlmStatus::Ready;
                Ok(())
            }
            Err(e) => {
                let msg = format!("Failed to load model: {}", e);
                *status_ref.status.write() = LlmStatus::Error(msg.clone());
                Err(msg)
            }
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn llm_unload_model(llm: State<'_, Arc<LlmState>>) -> Result<(), String> {
    *llm.engine.write() = None;
    *llm.status.write() = LlmStatus::Downloaded;
    Ok(())
}

#[tauri::command]
pub async fn llm_generate(
    llm: State<'_, Arc<LlmState>>,
    prompt: String,
    max_tokens: Option<usize>,
    temperature: Option<f64>,
) -> Result<String, String> {
    let status_ref = Arc::clone(&llm);

    tokio::task::spawn_blocking(move || {
        let mut engine_guard = status_ref.engine.write();
        let engine = engine_guard
            .as_mut()
            .ok_or_else(|| "Model not loaded".to_string())?;

        let prev_status = status_ref.status.read().clone();
        *status_ref.status.write() = LlmStatus::Generating;

        let result = engine.generate(&prompt, max_tokens.unwrap_or(100), temperature.unwrap_or(0.7));

        *status_ref.status.write() = prev_status;

        result.map_err(|e| format!("Generation failed: {}", e))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn llm_generate_branch_name(
    llm: State<'_, Arc<LlmState>>,
    description: String,
    use_tiny: Option<bool>,
) -> Result<String, String> {
    let status_ref = Arc::clone(&llm);

    tokio::task::spawn_blocking(move || {
        if use_tiny.unwrap_or(false) {
            // Use the tiny branch-name-gen engine
            status_ref
                .try_generate_branch_name(&description)
                .ok_or_else(|| "Tiny branch name engine not available".to_string())
        } else {
            // Use the full SmolLM2 engine
            let mut engine_guard = status_ref.engine.write();
            let engine = engine_guard
                .as_mut()
                .ok_or_else(|| "Model not loaded".to_string())?;

            let prev_status = status_ref.status.read().clone();
            *status_ref.status.write() = LlmStatus::Generating;

            let result = generate_branch_name(engine, &description);

            *status_ref.status.write() = prev_status;

            result.map_err(|e| format!("Branch name generation failed: {}", e))
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn llm_check_model_files(
    llm: State<'_, Arc<LlmState>>,
    subdir: Option<String>,
    gguf_filename: Option<String>,
    gguf_path: Option<String>,
    tokenizer_path: Option<String>,
) -> Result<bool, String> {
    let app_data_dir = llm
        .get_app_data_dir()
        .ok_or_else(|| "App data directory not initialized".to_string())?;

    let paths = match (gguf_path, tokenizer_path) {
        (Some(gguf), Some(tok)) => ModelPaths::from_paths(gguf.into(), tok.into()),
        _ => match (subdir, gguf_filename) {
            (Some(sub), Some(filename)) => ModelPaths::custom(&app_data_dir, &sub, &filename),
            _ => ModelPaths::new(&app_data_dir),
        },
    };

    Ok(paths.is_downloaded())
}
