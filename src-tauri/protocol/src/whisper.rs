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
    GetAudioLevel,
    SetVocabulary {
        terms: String,
    },
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
    VocabularyUpdated,
    AudioLevel {
        rms: f32,
        peak: f32,
        duration_ms: u64,
    },
    Error {
        message: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_ping_roundtrip() {
        let req = WhisperRequest::Ping;
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WhisperRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WhisperRequest::Ping));
    }

    #[test]
    fn request_shutdown_roundtrip() {
        let req = WhisperRequest::Shutdown;
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WhisperRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WhisperRequest::Shutdown));
    }

    #[test]
    fn request_start_recording_roundtrip() {
        let req = WhisperRequest::StartRecording { device_name: None };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WhisperRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WhisperRequest::StartRecording { .. }));
    }

    #[test]
    fn request_stop_recording_roundtrip() {
        let req = WhisperRequest::StopRecording;
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WhisperRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WhisperRequest::StopRecording));
    }

    #[test]
    fn request_get_status_roundtrip() {
        let req = WhisperRequest::GetStatus;
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WhisperRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WhisperRequest::GetStatus));
    }

    #[test]
    fn request_load_model_roundtrip() {
        let req = WhisperRequest::LoadModel {
            model_path: "/path/to/model.bin".to_string(),
            use_gpu: true,
            gpu_device: 1,
            language: "en".to_string(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WhisperRequest = serde_json::from_str(&json).unwrap();
        match parsed {
            WhisperRequest::LoadModel { model_path, use_gpu, gpu_device, language } => {
                assert_eq!(model_path, "/path/to/model.bin");
                assert!(use_gpu);
                assert_eq!(gpu_device, 1);
                assert_eq!(language, "en");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn request_get_audio_level_roundtrip() {
        let req = WhisperRequest::GetAudioLevel;
        let json = serde_json::to_string(&req).unwrap();
        let parsed: WhisperRequest = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WhisperRequest::GetAudioLevel));
    }

    #[test]
    fn response_pong_roundtrip() {
        let resp = WhisperResponse::Pong;
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: WhisperResponse = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, WhisperResponse::Pong));
    }

    #[test]
    fn response_transcription_result_roundtrip() {
        let resp = WhisperResponse::TranscriptionResult {
            text: "hello world".to_string(),
            duration_ms: 1234,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: WhisperResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            WhisperResponse::TranscriptionResult { text, duration_ms } => {
                assert_eq!(text, "hello world");
                assert_eq!(duration_ms, 1234);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn response_status_roundtrip() {
        let resp = WhisperResponse::Status {
            state: "idle".to_string(),
            model_loaded: true,
            model_name: Some("ggml-base.bin".to_string()),
            gpu_available: true,
            gpu_in_use: false,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: WhisperResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            WhisperResponse::Status { state, model_loaded, model_name, gpu_available, gpu_in_use } => {
                assert_eq!(state, "idle");
                assert!(model_loaded);
                assert_eq!(model_name, Some("ggml-base.bin".to_string()));
                assert!(gpu_available);
                assert!(!gpu_in_use);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn response_model_loaded_roundtrip() {
        let resp = WhisperResponse::ModelLoaded {
            model_name: "ggml-large.bin".to_string(),
            gpu_in_use: true,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: WhisperResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            WhisperResponse::ModelLoaded { model_name, gpu_in_use } => {
                assert_eq!(model_name, "ggml-large.bin");
                assert!(gpu_in_use);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn response_audio_level_roundtrip() {
        let resp = WhisperResponse::AudioLevel {
            rms: 0.25,
            peak: 0.8,
            duration_ms: 500,
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: WhisperResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            WhisperResponse::AudioLevel { rms, peak, duration_ms } => {
                assert!((rms - 0.25).abs() < 1e-6);
                assert!((peak - 0.8).abs() < 1e-6);
                assert_eq!(duration_ms, 500);
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn response_error_roundtrip() {
        let resp = WhisperResponse::Error {
            message: "Something went wrong".to_string(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: WhisperResponse = serde_json::from_str(&json).unwrap();
        match parsed {
            WhisperResponse::Error { message } => {
                assert_eq!(message, "Something went wrong");
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn request_tagged_serialization() {
        // Verify serde(tag = "type") works -- JSON should have a "type" field
        let json = serde_json::to_string(&WhisperRequest::Ping).unwrap();
        assert!(json.contains("\"type\":\"Ping\"") || json.contains("\"type\": \"Ping\""));
    }

    #[test]
    fn response_tagged_serialization() {
        let json = serde_json::to_string(&WhisperResponse::Pong).unwrap();
        assert!(json.contains("\"type\":\"Pong\"") || json.contains("\"type\": \"Pong\""));
    }
}
