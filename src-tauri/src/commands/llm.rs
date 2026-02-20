use std::sync::Arc;

use tauri::{Emitter, State};

use crate::llm_state::LlmState;
use godly_llm::{download_model, generate_branch_name, LlmEngine, LlmStatus, ModelPaths};

#[tauri::command]
pub async fn llm_get_status(llm: State<'_, Arc<LlmState>>) -> Result<LlmStatus, String> {
    Ok(llm.status.read().clone())
}

#[tauri::command]
pub async fn llm_download_model(
    app_handle: tauri::AppHandle,
    llm: State<'_, Arc<LlmState>>,
) -> Result<(), String> {
    let app_data_dir = llm
        .get_app_data_dir()
        .ok_or_else(|| "App data directory not initialized".to_string())?;

    *llm.status.write() = LlmStatus::Downloading { progress: 0.0 };

    let handle = app_handle.clone();
    let status_ref = Arc::clone(&llm);

    tokio::task::spawn_blocking(move || {
        let result = download_model(&app_data_dir, |downloaded, total| {
            let progress = if total > 0 {
                (downloaded as f32) / (total as f32)
            } else {
                0.0
            };
            let _ = handle.emit("llm-download-progress", progress);
        });

        match result {
            Ok(_) => {
                *status_ref.status.write() = LlmStatus::Downloaded;
                Ok(())
            }
            Err(e) => {
                let msg = format!("Download failed: {}", e);
                *status_ref.status.write() = LlmStatus::Error(msg.clone());
                Err(msg)
            }
        }
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn llm_load_model(llm: State<'_, Arc<LlmState>>) -> Result<(), String> {
    let app_data_dir = llm
        .get_app_data_dir()
        .ok_or_else(|| "App data directory not initialized".to_string())?;

    *llm.status.write() = LlmStatus::Loading;

    let status_ref = Arc::clone(&llm);

    tokio::task::spawn_blocking(move || {
        let paths = ModelPaths::new(&app_data_dir);
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
) -> Result<String, String> {
    let status_ref = Arc::clone(&llm);

    tokio::task::spawn_blocking(move || {
        let mut engine_guard = status_ref.engine.write();
        let engine = engine_guard
            .as_mut()
            .ok_or_else(|| "Model not loaded".to_string())?;

        let prev_status = status_ref.status.read().clone();
        *status_ref.status.write() = LlmStatus::Generating;

        let result = generate_branch_name(engine, &description);

        *status_ref.status.write() = prev_status;

        result.map_err(|e| format!("Branch name generation failed: {}", e))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}
