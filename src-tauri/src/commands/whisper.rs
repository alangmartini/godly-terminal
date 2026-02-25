use std::sync::Arc;

use tauri::State;

use crate::whisper_state::{WhisperConfig, WhisperRecordingState, WhisperState, WhisperStatus};

#[tauri::command]
pub async fn whisper_get_status(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<WhisperStatus, String> {
    Ok(whisper.get_status())
}

#[tauri::command]
pub async fn whisper_start_recording(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<(), String> {
    // For now, just update the state. The actual pipe IPC to godly-whisper
    // sidecar will be wired when the sidecar protocol is finalized.
    whisper.set_recording_state(WhisperRecordingState::Recording);
    Ok(())
}

#[tauri::command]
pub async fn whisper_stop_recording(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<String, String> {
    // Update state to transcribing
    whisper.set_recording_state(WhisperRecordingState::Transcribing);

    // TODO: Send StopRecording to sidecar via named pipe, wait for TranscriptionResult
    // For now, return empty string (sidecar integration pending)

    whisper.set_recording_state(WhisperRecordingState::Idle);
    Ok(String::new())
}

#[tauri::command]
pub async fn whisper_load_model(
    whisper: State<'_, Arc<WhisperState>>,
    model_name: String,
    use_gpu: bool,
    gpu_device: i32,
    language: String,
) -> Result<(), String> {
    let config = WhisperConfig {
        model_name: model_name.clone(),
        use_gpu,
        gpu_device,
        language,
    };
    whisper.set_config(config);

    // TODO: Send LoadModel to sidecar via named pipe
    // For now, mark as loaded optimistically
    whisper.set_model_loaded(true, Some(model_name));
    Ok(())
}

#[tauri::command]
pub async fn whisper_list_models(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<Vec<String>, String> {
    let models_dir = whisper
        .get_models_dir()
        .ok_or("App data directory not initialized")?;

    if !models_dir.exists() {
        return Ok(Vec::new());
    }

    let mut models = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&models_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("ggml-") && name.ends_with(".bin") {
                models.push(name);
            }
        }
    }
    models.sort();
    Ok(models)
}

#[tauri::command]
pub async fn whisper_get_config(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<WhisperConfig, String> {
    Ok(whisper.get_config())
}

#[tauri::command]
pub async fn whisper_set_config(
    whisper: State<'_, Arc<WhisperState>>,
    config: WhisperConfig,
) -> Result<(), String> {
    whisper.set_config(config);
    Ok(())
}
