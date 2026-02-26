use std::sync::Arc;

use tauri::State;

use crate::whisper_state::{WhisperConfig, WhisperRecordingState, WhisperState, WhisperStatus};

#[tauri::command]
pub async fn whisper_start_sidecar(
    app: tauri::AppHandle,
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<String, String> {
    // Check if already running
    if whisper.get_status().sidecar_running {
        return Ok("Sidecar already running".to_string());
    }

    let binary = crate::find_whisper_binary(&app).ok_or_else(|| {
        "godly-whisper binary not found. Place godly-whisper.exe next to the app binary.".to_string()
    })?;

    let pipe_name = whisper.get_pipe_name();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        let child = std::process::Command::new(&binary)
            .arg("--pipe")
            .arg(&pipe_name)
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start sidecar: {}", e))?;

        let pid = child.id();
        whisper.set_sidecar_running(true, Some(pid));
        Ok(format!("Sidecar started (PID {})", pid))
    }

    #[cfg(not(windows))]
    {
        Err("Voice sidecar is only supported on Windows".to_string())
    }
}

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
        microphone_device_id: None,
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
