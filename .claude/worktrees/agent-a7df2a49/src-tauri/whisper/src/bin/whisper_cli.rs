//! Minimal CLI tool for testing the whisper sidecar pipe directly — no Tauri app needed.
//!
//! Usage:
//!   whisper-cli [--pipe <NAME>] <command> [args...]
//!
//! Commands:
//!   ping                      Verify the sidecar is alive
//!   status                    Print sidecar status (model, recording state, GPU)
//!   load <model_path> [--gpu] Load a whisper model
//!   record <seconds>          Record for N seconds and print the transcription
//!   shutdown                  Ask the sidecar to exit

use std::fs::File;
use std::io;
use std::time::Duration;

use godly_protocol::{read_message, write_message, WhisperRequest, WhisperResponse};

fn connect(pipe_name: &str) -> io::Result<File> {
    #[cfg(windows)]
    {
        use std::os::windows::io::FromRawHandle;
        use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
        use winapi::um::handleapi::INVALID_HANDLE_VALUE;
        use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

        let wide: Vec<u16> = pipe_name.encode_utf16().chain(std::iter::once(0)).collect();
        let handle = unsafe {
            CreateFileW(
                wide.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };
        if handle == INVALID_HANDLE_VALUE {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { File::from_raw_handle(handle as _) })
    }

    #[cfg(not(windows))]
    {
        let _ = pipe_name;
        Err(io::Error::new(io::ErrorKind::Unsupported, "Only supported on Windows"))
    }
}

fn send(pipe: &mut File, req: &WhisperRequest) -> io::Result<WhisperResponse> {
    write_message(pipe, req)?;
    read_message::<_, WhisperResponse>(pipe)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Sidecar closed connection"))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut pipe_name = godly_protocol::whisper_pipe_name();
    let mut cmd_start = 1;

    // Parse --pipe <name> prefix
    if args.len() > 2 && args[1] == "--pipe" {
        pipe_name = args[2].clone();
        cmd_start = 3;
    }

    if cmd_start >= args.len() {
        eprintln!("Usage: whisper-cli [--pipe <NAME>] <ping|status|load|record|shutdown> [args...]");
        std::process::exit(1);
    }

    let command = &args[cmd_start];

    let mut pipe = match connect(&pipe_name) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Failed to connect to {}: {}", pipe_name, e);
            std::process::exit(1);
        }
    };

    match command.as_str() {
        "ping" => match send(&mut pipe, &WhisperRequest::Ping) {
            Ok(WhisperResponse::Pong) => println!("pong"),
            Ok(other) => println!("Unexpected: {:?}", other),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },

        "status" => match send(&mut pipe, &WhisperRequest::GetStatus) {
            Ok(WhisperResponse::Status { state, model_loaded, model_name, gpu_available, gpu_in_use }) => {
                println!("state:         {}", state);
                println!("model_loaded:  {}", model_loaded);
                println!("model_name:    {}", model_name.as_deref().unwrap_or("(none)"));
                println!("gpu_available: {}", gpu_available);
                println!("gpu_in_use:    {}", gpu_in_use);
            }
            Ok(WhisperResponse::Error { message }) => {
                eprintln!("Error: {}", message);
                std::process::exit(1);
            }
            Ok(other) => println!("Unexpected: {:?}", other),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },

        "load" => {
            if cmd_start + 1 >= args.len() {
                eprintln!("Usage: whisper-cli load <model_path> [--gpu]");
                std::process::exit(1);
            }
            let model_path = args[cmd_start + 1].clone();
            let use_gpu = args.iter().skip(cmd_start + 2).any(|a| a == "--gpu");

            match send(&mut pipe, &WhisperRequest::LoadModel {
                model_path,
                use_gpu,
                gpu_device: 0,
                language: String::new(),
            }) {
                Ok(WhisperResponse::ModelLoaded { model_name, gpu_in_use }) => {
                    println!("Loaded: {} (GPU: {})", model_name, gpu_in_use);
                }
                Ok(WhisperResponse::Error { message }) => {
                    eprintln!("Error: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => println!("Unexpected: {:?}", other),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "record" => {
            let seconds: u64 = args.get(cmd_start + 1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(3);

            // Start recording
            match send(&mut pipe, &WhisperRequest::StartRecording { device_name: None }) {
                Ok(WhisperResponse::RecordingStarted) => println!("Recording for {}s...", seconds),
                Ok(WhisperResponse::Error { message }) => {
                    eprintln!("Error: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => {
                    eprintln!("Unexpected: {:?}", other);
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }

            std::thread::sleep(Duration::from_secs(seconds));

            // Stop recording
            match send(&mut pipe, &WhisperRequest::StopRecording) {
                Ok(WhisperResponse::TranscriptionResult { text, duration_ms }) => {
                    if text.is_empty() {
                        println!("(no speech detected) [{}ms]", duration_ms);
                    } else {
                        println!("{} [{}ms]", text, duration_ms);
                    }
                }
                Ok(WhisperResponse::Error { message }) => {
                    eprintln!("Error: {}", message);
                    std::process::exit(1);
                }
                Ok(other) => println!("Unexpected: {:?}", other),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    std::process::exit(1);
                }
            }
        }

        "shutdown" => match send(&mut pipe, &WhisperRequest::Shutdown) {
            Ok(_) => println!("Shutdown sent"),
            Err(_) => println!("Shutdown sent (connection closed)"),
        },

        _ => {
            eprintln!("Unknown command: {}", command);
            eprintln!("Commands: ping, status, load, record, shutdown");
            std::process::exit(1);
        }
    }
}
