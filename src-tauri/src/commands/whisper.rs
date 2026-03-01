use std::sync::Arc;

use godly_protocol::{AudioDeviceInfo, WhisperRequest, WhisperResponse};
use godly_renderer::GpuAdapterInfo;
use tauri::{Emitter, State};

use crate::whisper_state::{WhisperConfig, WhisperRecordingState, WhisperState, WhisperStatus};

/// Spawn the whisper sidecar binary, connect to its pipe, and verify with a ping.
/// Shared by `whisper_start_sidecar` and `whisper_restart_sidecar`.
fn start_sidecar_inner(
    app: &tauri::AppHandle,
    whisper: &WhisperState,
) -> Result<String, String> {
    let binary = crate::find_whisper_binary(app).ok_or_else(|| {
        "godly-whisper binary not found. Place godly-whisper.exe next to the app binary.".to_string()
    })?;

    let pipe_name = whisper.get_pipe_name().to_string();
    let models_dir = whisper.get_models_dir()
        .ok_or("App data directory not initialized")?;

    let instance = std::env::var("GODLY_INSTANCE").unwrap_or_default();

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        let mut cmd = std::process::Command::new(&binary);
        cmd.arg("--pipe").arg(&pipe_name)
            .arg("--models-dir").arg(models_dir.to_string_lossy().as_ref());

        if !instance.is_empty() {
            cmd.arg("--instance").arg(&instance);
        }

        let child = cmd
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| format!("Failed to start sidecar: {}", e))?;

        let pid = child.id();
        whisper.set_sidecar_running(true, Some(pid));

        // Retry connecting to the pipe (sidecar needs time to create it)
        let mut connected = false;
        for _ in 0..20 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if whisper.client().connect(&pipe_name).is_ok() {
                connected = true;
                break;
            }
        }

        if !connected {
            return Err("Sidecar started but failed to connect to pipe".to_string());
        }

        // Verify with a ping
        match whisper.client().send_request(&WhisperRequest::Ping) {
            Ok(WhisperResponse::Pong) => {}
            Ok(other) => return Err(format!("Unexpected ping response: {:?}", other)),
            Err(e) => return Err(format!("Ping failed: {}", e)),
        }

        Ok(format!("Sidecar started (PID {})", pid))
    }

    #[cfg(not(windows))]
    {
        Err("Voice sidecar is only supported on Windows".to_string())
    }
}

/// Kill a process by PID using the Windows TerminateProcess API.
#[cfg(windows)]
fn kill_process_by_pid(pid: u32) -> Result<(), String> {
    use winapi::um::processthreadsapi::{OpenProcess, TerminateProcess};
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::winnt::PROCESS_TERMINATE;

    unsafe {
        let handle = OpenProcess(PROCESS_TERMINATE, 0, pid);
        if handle.is_null() {
            return Err(format!("Failed to open process {}: {}", pid, std::io::Error::last_os_error()));
        }
        let result = TerminateProcess(handle, 1);
        CloseHandle(handle);
        if result == 0 {
            return Err(format!("Failed to terminate process {}: {}", pid, std::io::Error::last_os_error()));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn whisper_start_sidecar(
    app: tauri::AppHandle,
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<String, String> {
    // Check if already running and connected
    if whisper.get_status().sidecar_running && whisper.client().is_connected() {
        return Ok("Sidecar already running".to_string());
    }

    start_sidecar_inner(&app, &whisper)
}

#[tauri::command]
pub async fn whisper_restart_sidecar(
    app: tauri::AppHandle,
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<String, String> {
    // 1. Try graceful shutdown via pipe
    let graceful = whisper.client().send_request(&WhisperRequest::Shutdown);
    if graceful.is_err() {
        // 2. Fallback: kill by PID
        #[cfg(windows)]
        if let Some(pid) = whisper.get_sidecar_pid() {
            let _ = kill_process_by_pid(pid);
        }
    }

    // 3. Disconnect client and reset state
    whisper.client().disconnect();
    whisper.set_sidecar_running(false, None);
    whisper.set_model_loaded(false, None);
    whisper.set_recording_state(WhisperRecordingState::Idle);

    // 4. Wait for pipe to be released
    std::thread::sleep(std::time::Duration::from_millis(200));

    // 5. Spawn new sidecar
    start_sidecar_inner(&app, &whisper)
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
    let config = whisper.get_config();
    let resp = whisper.client().send_request(&WhisperRequest::StartRecording {
        device_name: config.microphone_device_id.clone(),
    })
        .map_err(|e| format!("Failed to send StartRecording: {}", e))?;

    match resp {
        WhisperResponse::RecordingStarted => {
            whisper.set_recording_state(WhisperRecordingState::Recording);
            Ok(())
        }
        WhisperResponse::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub duration_ms: u64,
}

#[tauri::command]
pub async fn whisper_stop_recording(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<TranscriptionResult, String> {
    whisper.set_recording_state(WhisperRecordingState::Transcribing);

    let resp = whisper.client().send_request(&WhisperRequest::StopRecording)
        .map_err(|e| {
            whisper.set_recording_state(WhisperRecordingState::Idle);
            format!("Failed to send StopRecording: {}", e)
        })?;

    match resp {
        WhisperResponse::TranscriptionResult { text, duration_ms } => {
            whisper.set_recording_state(WhisperRecordingState::Idle);
            Ok(TranscriptionResult { text, duration_ms })
        }
        WhisperResponse::Error { message } => {
            whisper.set_recording_state(WhisperRecordingState::Idle);
            Err(message)
        }
        other => {
            whisper.set_recording_state(WhisperRecordingState::Idle);
            Err(format!("Unexpected response: {:?}", other))
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLevelInfo {
    pub rms: f32,
    pub peak: f32,
    pub duration_ms: u64,
}

#[tauri::command]
pub async fn whisper_get_audio_level(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<AudioLevelInfo, String> {
    let resp = whisper.client().send_request(&WhisperRequest::GetAudioLevel)
        .map_err(|e| format!("Failed to get audio level: {}", e))?;

    match resp {
        WhisperResponse::AudioLevel { rms, peak, duration_ms } => {
            Ok(AudioLevelInfo { rms, peak, duration_ms })
        }
        WhisperResponse::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn whisper_load_model(
    whisper: State<'_, Arc<WhisperState>>,
    model_name: String,
    use_gpu: bool,
    gpu_device: i32,
    language: String,
) -> Result<(), String> {
    let prev_config = whisper.get_config();
    let config = WhisperConfig {
        model_name: model_name.clone(),
        use_gpu,
        gpu_device,
        language: language.clone(),
        microphone_device_id: prev_config.microphone_device_id,
        custom_vocabulary: prev_config.custom_vocabulary.clone(),
    };
    whisper.set_config(config);

    let model_path = whisper.get_models_dir()
        .ok_or("App data directory not initialized")?
        .join(&model_name)
        .to_string_lossy()
        .to_string();

    let resp = whisper.client().send_request(&WhisperRequest::LoadModel {
        model_path,
        use_gpu,
        gpu_device,
        language,
    }).map_err(|e| format!("Failed to send LoadModel: {}", e))?;

    match resp {
        WhisperResponse::ModelLoaded { model_name: name, gpu_in_use } => {
            whisper.set_model_loaded(true, Some(name));
            let mut status = whisper.get_status();
            status.gpu_in_use = gpu_in_use;
            // Push custom vocabulary to sidecar after model load
            if !prev_config.custom_vocabulary.is_empty() {
                let _ = whisper.client().send_request(&WhisperRequest::SetVocabulary {
                    terms: prev_config.custom_vocabulary,
                });
            }
            Ok(())
        }
        WhisperResponse::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
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

#[tauri::command]
pub async fn whisper_set_vocabulary(
    whisper: State<'_, Arc<WhisperState>>,
    terms: String,
) -> Result<(), String> {
    // Update config
    let mut config = whisper.get_config();
    config.custom_vocabulary = terms.clone();
    whisper.set_config(config);

    // Push to sidecar if connected
    if whisper.get_status().sidecar_running {
        let resp = whisper.client().send_request(&WhisperRequest::SetVocabulary { terms })
            .map_err(|e| format!("Failed to send SetVocabulary: {}", e))?;
        match resp {
            WhisperResponse::VocabularyUpdated => Ok(()),
            WhisperResponse::Error { message } => Err(message),
            other => Err(format!("Unexpected response: {:?}", other)),
        }
    } else {
        Ok(())
    }
}

#[tauri::command]
pub async fn list_gpu_devices() -> Result<Vec<GpuAdapterInfo>, String> {
    Ok(godly_renderer::enumerate_gpu_adapters())
}

#[tauri::command]
pub async fn whisper_list_audio_devices(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<Vec<AudioDeviceInfo>, String> {
    let resp = whisper.client().send_request(&WhisperRequest::ListAudioDevices)
        .map_err(|e| format!("Failed to send ListAudioDevices: {}", e))?;

    match resp {
        WhisperResponse::AudioDeviceList { devices } => Ok(devices),
        WhisperResponse::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn whisper_playback_recording(
    whisper: State<'_, Arc<WhisperState>>,
) -> Result<(), String> {
    let resp = whisper.client().send_request(&WhisperRequest::PlaybackLastRecording)
        .map_err(|e| format!("Failed to send PlaybackLastRecording: {}", e))?;

    match resp {
        WhisperResponse::PlaybackComplete => Ok(()),
        WhisperResponse::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

#[tauri::command]
pub async fn whisper_download_model(
    app: tauri::AppHandle,
    whisper: State<'_, Arc<WhisperState>>,
    model_name: String,
) -> Result<(), String> {
    let models_dir = whisper
        .get_models_dir()
        .ok_or("App data directory not initialized")?;

    // Create models dir if it doesn't exist
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models directory: {}", e))?;

    let dest = models_dir.join(&model_name);

    // Already downloaded?
    if dest.exists() {
        return Ok(());
    }

    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        model_name
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Download request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", resp.status()));
    }

    let total = resp.content_length().unwrap_or(0);
    let _ = app.emit("whisper-download-progress", serde_json::json!({
        "model": model_name,
        "downloaded": 0u64,
        "total": total,
        "phase": "downloading",
    }));

    // Download to a temp file first, then rename (atomic-ish)
    let tmp_dest = models_dir.join(format!("{}.downloading", model_name));
    let mut file = tokio::fs::File::create(&tmp_dest)
        .await
        .map_err(|e| format!("Failed to create temp file: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let mut stream = resp;

    loop {
        let chunk = stream
            .chunk()
            .await
            .map_err(|e| format!("Download interrupted: {}", e))?;

        match chunk {
            Some(bytes) => {
                use tokio::io::AsyncWriteExt;
                file.write_all(&bytes)
                    .await
                    .map_err(|e| format!("Failed to write: {}", e))?;
                downloaded += bytes.len() as u64;

                // Emit progress at most every 100ms
                if last_emit.elapsed() >= std::time::Duration::from_millis(100) {
                    let _ = app.emit("whisper-download-progress", serde_json::json!({
                        "model": model_name,
                        "downloaded": downloaded,
                        "total": total,
                        "phase": "downloading",
                    }));
                    last_emit = std::time::Instant::now();
                }
            }
            None => break,
        }
    }

    // Flush and rename
    {
        use tokio::io::AsyncWriteExt;
        file.flush().await.map_err(|e| format!("Flush failed: {}", e))?;
    }
    drop(file);

    std::fs::rename(&tmp_dest, &dest)
        .map_err(|e| format!("Failed to finalize download: {}", e))?;

    let _ = app.emit("whisper-download-progress", serde_json::json!({
        "model": model_name,
        "downloaded": total,
        "total": total,
        "phase": "complete",
    }));

    Ok(())
}
