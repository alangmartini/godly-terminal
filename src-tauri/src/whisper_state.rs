use std::path::PathBuf;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum WhisperRecordingState {
    Idle,
    Recording,
    Transcribing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperStatus {
    pub state: WhisperRecordingState,
    pub model_loaded: bool,
    pub model_name: Option<String>,
    pub gpu_available: bool,
    pub gpu_in_use: bool,
    pub sidecar_running: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperConfig {
    pub model_name: String,
    pub language: String,
    pub use_gpu: bool,
    pub gpu_device: i32,
}

impl Default for WhisperConfig {
    fn default() -> Self {
        Self {
            model_name: "ggml-base.bin".to_string(),
            language: String::new(), // empty = auto-detect
            use_gpu: true,
            gpu_device: 0,
        }
    }
}

/// Named pipe IPC protocol messages sent to the godly-whisper sidecar.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WhisperRequest {
    Ping,
    StartRecording,
    StopRecording,
    GetStatus,
    LoadModel {
        model_name: String,
        use_gpu: bool,
        gpu_device: i32,
        language: String,
    },
    ListModels,
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
        gpu_available: bool,
    },
    ModelLoaded,
    ModelList {
        models: Vec<String>,
    },
    Error {
        message: String,
    },
}

pub struct WhisperState {
    config: RwLock<WhisperConfig>,
    status: RwLock<WhisperStatus>,
    app_data_dir: RwLock<Option<PathBuf>>,
    sidecar_pid: RwLock<Option<u32>>,
    pipe_name: RwLock<String>,
}

impl WhisperState {
    pub fn new() -> Self {
        let instance = std::env::var("GODLY_INSTANCE").unwrap_or_default();
        let pipe_name = if instance.is_empty() {
            r"\\.\pipe\godly-whisper".to_string()
        } else {
            format!(r"\\.\pipe\godly-whisper-{}", instance)
        };

        Self {
            config: RwLock::new(WhisperConfig::default()),
            status: RwLock::new(WhisperStatus {
                state: WhisperRecordingState::Idle,
                model_loaded: false,
                model_name: None,
                gpu_available: false,
                gpu_in_use: false,
                sidecar_running: false,
            }),
            app_data_dir: RwLock::new(None),
            sidecar_pid: RwLock::new(None),
            pipe_name: RwLock::new(pipe_name),
        }
    }

    /// Initialize with app data dir for model storage.
    pub fn init(&self, app_data_dir: PathBuf) {
        *self.app_data_dir.write() = Some(app_data_dir);
    }

    pub fn get_config(&self) -> WhisperConfig {
        self.config.read().clone()
    }

    pub fn set_config(&self, config: WhisperConfig) {
        *self.config.write() = config;
    }

    pub fn get_status(&self) -> WhisperStatus {
        self.status.read().clone()
    }

    pub fn set_recording_state(&self, state: WhisperRecordingState) {
        self.status.write().state = state;
    }

    pub fn set_sidecar_running(&self, running: bool, pid: Option<u32>) {
        self.status.write().sidecar_running = running;
        *self.sidecar_pid.write() = pid;
    }

    pub fn set_model_loaded(&self, loaded: bool, name: Option<String>) {
        let mut status = self.status.write();
        status.model_loaded = loaded;
        status.model_name = name;
    }

    pub fn get_pipe_name(&self) -> String {
        self.pipe_name.read().clone()
    }

    pub fn get_models_dir(&self) -> Option<PathBuf> {
        self.app_data_dir
            .read()
            .as_ref()
            .map(|d| d.join("whisper-models"))
    }

    pub fn get_sidecar_pid(&self) -> Option<u32> {
        *self.sidecar_pid.read()
    }
}
