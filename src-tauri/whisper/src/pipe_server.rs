use std::fs::File;
use std::io;

use godly_protocol::{read_message, write_message, WhisperRequest, WhisperResponse};

use crate::audio::AudioRecorder;
use crate::transcribe::Transcriber;

/// Create a named pipe server instance and wait for a client to connect.
#[cfg(windows)]
pub fn create_pipe_server(pipe_name: &str) -> io::Result<File> {
    use std::os::windows::io::FromRawHandle;
    use winapi::um::namedpipeapi::{ConnectNamedPipe, CreateNamedPipeW};
    use winapi::um::winbase::{PIPE_ACCESS_DUPLEX, PIPE_TYPE_BYTE, PIPE_WAIT};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;

    let wide_name: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();

    let handle = unsafe {
        CreateNamedPipeW(
            wide_name.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_WAIT,
            1, // max instances
            64 * 1024,
            64 * 1024,
            0,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    // Wait for client to connect
    let result = unsafe { ConnectNamedPipe(handle, std::ptr::null_mut()) };
    if result == 0 {
        let err = io::Error::last_os_error();
        // ERROR_PIPE_CONNECTED (535) means client connected between Create and Connect
        if err.raw_os_error() != Some(535) {
            return Err(err);
        }
    }

    Ok(unsafe { File::from_raw_handle(handle as _) })
}

#[cfg(not(windows))]
pub fn create_pipe_server(_pipe_name: &str) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "Named pipe server is only supported on Windows",
    ))
}

/// Run the request-response loop on a connected pipe.
pub fn handle_client(
    pipe: &mut File,
    recorder: &mut AudioRecorder,
    transcriber: &mut Transcriber,
) -> io::Result<()> {
    loop {
        let request: WhisperRequest = match read_message(pipe)? {
            Some(req) => req,
            None => return Ok(()), // EOF — client disconnected
        };

        let response = handle_request(request, recorder, transcriber);
        write_message(pipe, &response)?;

        // If we just handled a Shutdown, exit after sending the response
        if matches!(response, WhisperResponse::Pong) {
            // Pong is fine, continue
        }
    }
}

fn handle_request(
    request: WhisperRequest,
    recorder: &mut AudioRecorder,
    transcriber: &mut Transcriber,
) -> WhisperResponse {
    match request {
        WhisperRequest::Ping => WhisperResponse::Pong,

        WhisperRequest::Shutdown => {
            // Caller will handle the actual exit after sending response
            eprintln!("[whisper] Shutdown requested");
            std::process::exit(0);
        }

        WhisperRequest::GetStatus => WhisperResponse::Status {
            state: if recorder.is_recording() {
                "recording".to_string()
            } else {
                "idle".to_string()
            },
            model_loaded: transcriber.is_loaded(),
            model_name: transcriber.model_name().map(|s| s.to_string()),
            gpu_available: true, // simplified — whisper.cpp checks at load time
            gpu_in_use: transcriber.gpu_in_use(),
        },

        WhisperRequest::LoadModel {
            model_path,
            use_gpu,
            gpu_device,
            language,
        } => match transcriber.load_model(&model_path, use_gpu, gpu_device, language) {
            Ok((model_name, gpu_in_use)) => {
                eprintln!("[whisper] Model loaded: {} (GPU: {})", model_name, gpu_in_use);
                WhisperResponse::ModelLoaded {
                    model_name,
                    gpu_in_use,
                }
            }
            Err(e) => {
                eprintln!("[whisper] Model load failed: {}", e);
                WhisperResponse::Error { message: e }
            }
        },

        WhisperRequest::StartRecording => {
            if !transcriber.is_loaded() {
                return WhisperResponse::Error {
                    message: "No model loaded".to_string(),
                };
            }
            match recorder.start() {
                Ok(()) => {
                    eprintln!("[whisper] Recording started");
                    WhisperResponse::RecordingStarted
                }
                Err(e) => {
                    eprintln!("[whisper] Failed to start recording: {}", e);
                    WhisperResponse::Error { message: e }
                }
            }
        }

        WhisperRequest::StopRecording => {
            let samples = match recorder.stop() {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[whisper] Failed to stop recording: {}", e);
                    return WhisperResponse::Error { message: e };
                }
            };

            eprintln!("[whisper] Recording stopped, {} samples captured", samples.len());

            if samples.is_empty() {
                return WhisperResponse::TranscriptionResult {
                    text: String::new(),
                    duration_ms: 0,
                };
            }

            match transcriber.transcribe(&samples) {
                Ok((text, duration_ms)) => {
                    eprintln!("[whisper] Transcription done in {}ms: {:?}", duration_ms, text);
                    WhisperResponse::TranscriptionResult { text, duration_ms }
                }
                Err(e) => {
                    eprintln!("[whisper] Transcription failed: {}", e);
                    WhisperResponse::Error { message: e }
                }
            }
        }
    }
}
