use serde::{Deserialize, Serialize};

/// Named pipe IPC protocol messages sent to the godly-whisper sidecar.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WhisperRequest {
    Ping,
    Shutdown,
    StartRecording {
        device_name: Option<String>,
    },
    StopRecording,
    GetStatus,
    LoadModel {
        model_path: String,
        use_gpu: bool,
        gpu_device: i32,
        language: String,
    },
    ListAudioDevices,
    PlaybackLastRecording,
}

/// Info about an available audio input device.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
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
    AudioDeviceList {
        devices: Vec<AudioDeviceInfo>,
    },
    PlaybackComplete,
    Error {
        message: String,
    },
}
