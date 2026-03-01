//! Sidecar binary management: finding and spawning companion binaries
//! (godly-mcp, godly-remote, godly-whisper) and cleaning up stale `.old` files.

use tauri::Manager;

/// Delete `.old` binaries left in the resource directory from previous upgrades.
/// During builds and installs, locked executables are renamed to `.old` so new
/// binaries can be written. This cleans them up once the old processes have exited.
pub(crate) fn cleanup_old_binaries(app_handle: &tauri::AppHandle) {
    let resource_dir = match app_handle.path().resource_dir() {
        Ok(dir) => dir,
        Err(_) => return,
    };

    for name in &[
        "godly-daemon.exe.old",
        "godly-mcp.exe.old",
        "godly-notify.exe.old",
        "godly-remote.exe.old",
        "godly-whisper.exe.old",
    ] {
        let path = resource_dir.join(name);
        if path.exists() {
            match std::fs::remove_file(&path) {
                Ok(_) => eprintln!("[sidecar] Cleaned up {}", name),
                Err(e) => eprintln!("[sidecar] Could not clean up {} (still locked?): {}", name, e),
            }
        }
    }
}

/// Find the godly-mcp binary: resource dir (installed) > exe dir > target/debug (dev).
pub(crate) fn find_mcp_binary(app_handle: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    // 1. Resource dir (Tauri bundle)
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        let p = resource_dir.join("godly-mcp.exe");
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Same dir as current exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("godly-mcp.exe");
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 3. target/debug (dev builds)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("../godly-mcp.exe");
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Start the MCP HTTP server as a detached process if not already running.
pub(crate) fn start_mcp_http_server(app_handle: &tauri::AppHandle) {
    // Check if a server is already running via discovery file
    if let Ok(appdata) = std::env::var("APPDATA") {
        let discovery = std::path::PathBuf::from(&appdata)
            .join("com.godly.terminal")
            .join("mcp-http.json");

        if discovery.exists() {
            if let Ok(content) = std::fs::read_to_string(&discovery) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(pid) = json.get("pid").and_then(|v| v.as_u64()) {
                        // Check if process is still alive
                        #[cfg(windows)]
                        {
                            use winapi::um::handleapi::CloseHandle;
                            use winapi::um::processthreadsapi::OpenProcess;
                            use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;

                            let handle = unsafe {
                                OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid as u32)
                            };
                            if !handle.is_null() {
                                unsafe { CloseHandle(handle) };
                                eprintln!(
                                    "[sidecar] MCP HTTP server already running (PID {}), skipping spawn",
                                    pid
                                );
                                return;
                            }
                        }
                    }
                }
            }
            // Stale discovery file, remove it
            let _ = std::fs::remove_file(&discovery);
        }
    }

    let mcp_binary = match find_mcp_binary(app_handle) {
        Some(p) => p,
        None => {
            eprintln!("[sidecar] godly-mcp binary not found, skipping HTTP server start");
            return;
        }
    };

    eprintln!(
        "[sidecar] Starting MCP HTTP server: {}",
        mcp_binary.display()
    );

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        match std::process::Command::new(&mcp_binary)
            .arg("--http")
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => {
                eprintln!("[sidecar] MCP HTTP server spawned (PID {})", child.id());
            }
            Err(e) => {
                eprintln!("[sidecar] Failed to start MCP HTTP server: {}", e);
            }
        }
    }

    #[cfg(not(windows))]
    {
        eprintln!("[sidecar] MCP HTTP server auto-start is only supported on Windows");
    }
}

/// Find the godly-remote binary: resource dir (installed) > exe dir > target/debug (dev).
pub(crate) fn find_remote_binary(app_handle: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    // 1. Resource dir (Tauri bundle)
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        let p = resource_dir.join("godly-remote.exe");
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Same dir as current exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("godly-remote.exe");
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 3. target/debug (dev builds)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("../godly-remote.exe");
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Find the godly-whisper binary: resource dir (installed) > exe dir > target/debug (dev).
pub(crate) fn find_whisper_binary(app_handle: &tauri::AppHandle) -> Option<std::path::PathBuf> {
    // 1. Resource dir (Tauri bundle)
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        let p = resource_dir.join("godly-whisper.exe");
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Same dir as current exe
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("godly-whisper.exe");
            if p.exists() {
                return Some(p);
            }
        }
    }

    // 3. target/debug (dev builds)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join("../godly-whisper.exe");
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Start the Remote HTTP server as a detached process.
/// godly-remote doesn't write a discovery file — if the port is already bound
/// (from a previous app launch), the new process simply fails to bind and exits.
pub(crate) fn start_remote_http_server(app_handle: &tauri::AppHandle) {
    let remote_binary = match find_remote_binary(app_handle) {
        Some(p) => p,
        None => {
            eprintln!("[sidecar] godly-remote binary not found, skipping HTTP server start");
            return;
        }
    };

    eprintln!(
        "[sidecar] Starting Remote HTTP server: {}",
        remote_binary.display()
    );

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        const DETACHED_PROCESS: u32 = 0x00000008;

        match std::process::Command::new(&remote_binary)
            .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(child) => {
                eprintln!("[sidecar] Remote HTTP server spawned (PID {})", child.id());
            }
            Err(e) => {
                eprintln!("[sidecar] Failed to start Remote HTTP server: {}", e);
            }
        }
    }

    #[cfg(not(windows))]
    {
        eprintln!("[sidecar] Remote HTTP server auto-start is only supported on Windows");
    }
}
