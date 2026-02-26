use serde::{Deserialize, Serialize};

/// Named pipe IPC protocol messages sent to the godly-whisper sidecar.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WhisperRequest {
    Ping,
    Shutdown,
    StartRecording,
    StopRecording,
    GetStatus,
    LoadModel {
        model_path: String,
        use_gpu: bool,
        gpu_device: i32,
        language: String,
    },
}

/// Named pipe IPC protocol messages received from the godly-whisper sidecar.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WhisperResponse {
    Pong,
    RecordingStarted,
    TranscriptionResult {
        text: String,
        duration_ms: u64,
    },
    Status {
        state: String,
        model_loaded: bool,
        model_name: Option<String>,
        gpu_available: bool,
        gpu_in_use: bool,
    },
    ModelLoaded {
        model_name: String,
        gpu_in_use: bool,
    },
    Error {
        message: String,
    },
}
