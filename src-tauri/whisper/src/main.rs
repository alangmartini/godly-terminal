mod audio;
mod pipe_server;
mod transcribe;

use audio::AudioRecorder;
use transcribe::Transcriber;

const BUILD: u32 = 2;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let mut pipe_name = godly_protocol::whisper_pipe_name();
    let mut models_dir = String::new();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--pipe" => {
                i += 1;
                if i < args.len() {
                    pipe_name = args[i].clone();
                }
            }
            "--instance" => {
                i += 1;
                // Instance is handled via GODLY_INSTANCE env var by the caller,
                // but we accept it as CLI arg for completeness
                if i < args.len() {
                    std::env::set_var("GODLY_INSTANCE", &args[i]);
                    pipe_name = godly_protocol::whisper_pipe_name();
                }
            }
            "--models-dir" => {
                i += 1;
                if i < args.len() {
                    models_dir = args[i].clone();
                }
            }
            "--help" | "-h" => {
                eprintln!("godly-whisper — Whisper speech-to-text sidecar for Godly Terminal");
                eprintln!();
                eprintln!("USAGE:");
                eprintln!("    godly-whisper [OPTIONS]");
                eprintln!();
                eprintln!("OPTIONS:");
                eprintln!("    --pipe <NAME>        Named pipe path (default: auto from GODLY_INSTANCE)");
                eprintln!("    --instance <NAME>    Instance name for pipe isolation");
                eprintln!("    --models-dir <PATH>  Directory containing whisper model files");
                eprintln!("    --help               Show this help");
                std::process::exit(0);
            }
            _ => {
                eprintln!("[whisper] Unknown argument: {}", args[i]);
            }
        }
        i += 1;
    }

    eprintln!("=== godly-whisper starting === build={} cuda={}", BUILD, cfg!(feature = "cuda"));
    eprintln!("[whisper] Pipe: {}", pipe_name);
    if !models_dir.is_empty() {
        eprintln!("[whisper] Models dir: {}", models_dir);
    }

    // Detach from console when not in debug mode
    #[cfg(windows)]
    if std::env::var("GODLY_NO_DETACH").is_err() {
        unsafe {
            winapi::um::wincon::FreeConsole();
        }
        // Redirect stdio to NUL to avoid broken pipe errors
        redirect_stdio_to_nul();
    }

    let mut recorder = AudioRecorder::new();
    let mut transcriber = Transcriber::new();

    eprintln!("[whisper] Creating pipe server...");
    loop {
        // Create pipe and wait for client
        let mut pipe = match pipe_server::create_pipe_server(&pipe_name) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("[whisper] Failed to create pipe: {}", e);
                std::thread::sleep(std::time::Duration::from_secs(1));
                continue;
            }
        };

        eprintln!("[whisper] Client connected");

        // Handle requests until client disconnects
        match pipe_server::handle_client(&mut pipe, &mut recorder, &mut transcriber) {
            Ok(()) => eprintln!("[whisper] Client disconnected"),
            Err(e) => eprintln!("[whisper] Client error: {}", e),
        }

        // Drop the pipe to allow re-creation
        drop(pipe);
    }
}

/// Redirect stdout/stderr to NUL on Windows (for detached mode).
#[cfg(windows)]
fn redirect_stdio_to_nul() {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;

    if let Ok(nul) = OpenOptions::new().write(true).open("NUL") {
        let nul_handle = nul.as_raw_handle();
        unsafe {
            use winapi::um::processenv::SetStdHandle;
            use winapi::um::winbase::{STD_OUTPUT_HANDLE, STD_ERROR_HANDLE};
            SetStdHandle(STD_OUTPUT_HANDLE, nul_handle as _);
            SetStdHandle(STD_ERROR_HANDLE, nul_handle as _);
        }
        // Keep the NUL file open for the lifetime of the process
        std::mem::forget(nul);
    }
}
