// Prevents additional console window on Windows in release
#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

fn main() {
    let mode = godly_protocol::frontend_mode();

    match mode {
        godly_protocol::FrontendMode::Native => {
            // Try to launch the native Iced shell binary.
            // If it's not found, fall back to the web frontend.
            match launch_native() {
                Ok(()) => {} // Native shell launched, this process can exit
                Err(e) => {
                    eprintln!("Failed to launch native frontend: {}. Falling back to web.", e);
                    godly_terminal_lib::run();
                }
            }
        }
        _ => {
            // Web or Shadow mode — use the Tauri frontend
            godly_terminal_lib::run();
        }
    }
}

/// Find and launch the native Iced shell binary (godly-native.exe).
/// Returns Ok(()) if the native binary was found and launched.
fn launch_native() -> Result<(), String> {
    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Failed to get current exe path: {}", e))?;
    let exe_dir = current_exe
        .parent()
        .ok_or("No parent directory for current exe")?;

    let native_name = if cfg!(windows) {
        "godly-native.exe"
    } else {
        "godly-native"
    };
    let native_path = exe_dir.join(native_name);

    if !native_path.exists() {
        return Err(format!("Native binary not found at {:?}", native_path));
    }

    // Launch the native binary as a detached process.
    // Pass through any relevant environment variables.
    let mut cmd = std::process::Command::new(&native_path);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // DETACHED_PROCESS (0x00000008) — don't inherit the console
        cmd.creation_flags(0x00000008);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn native binary: {}", e))?;

    // The native binary is running independently — drop our handle.
    drop(child);
    Ok(())
}
