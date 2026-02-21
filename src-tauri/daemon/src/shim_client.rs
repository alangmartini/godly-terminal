//! Client for communicating with pty-shim processes.
//!
//! The daemon spawns pty-shim as a detached process for each terminal session.
//! The shim holds the ConPTY handles and survives daemon crashes. The daemon
//! connects to the shim via a named pipe to exchange input/output/control data.

use std::io;
use std::process::Command;

use godly_protocol::types::ShellType;
use godly_protocol::ShimMetadata;

use crate::debug_log::daemon_log;

/// Spawn a new pty-shim process for the given session.
/// Returns the ShimMetadata on success.
pub fn spawn_shim(
    session_id: &str,
    shell_type: &ShellType,
    cwd: Option<&str>,
    rows: u16,
    cols: u16,
    env: Option<&std::collections::HashMap<String, String>>,
) -> Result<ShimMetadata, String> {
    let pipe_name = godly_protocol::shim_pipe_name(session_id);

    // Find the shim binary next to the daemon binary
    let shim_path = find_shim_binary()?;

    // Convert ShellType to string arg for the shim
    let shell_type_str = shell_type_to_shim_arg(shell_type);

    let mut cmd = Command::new(&shim_path);
    cmd.args(["--session-id", session_id]);
    cmd.args(["--shell-type", &shell_type_str]);
    cmd.args(["--rows", &rows.to_string()]);
    cmd.args(["--cols", &cols.to_string()]);
    cmd.args(["--pipe-name", &pipe_name]);

    if let Some(dir) = cwd {
        cmd.args(["--cwd", dir]);
    }

    // Propagate GODLY_INSTANCE if set (for pipe name isolation in tests)
    if let Ok(instance) = std::env::var("GODLY_INSTANCE") {
        cmd.env("GODLY_INSTANCE", &instance);
    }

    // Propagate custom environment variables
    if let Some(env_vars) = env {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }

    // Spawn detached: the shim must outlive the daemon
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP (0x00000200) | CREATE_NO_WINDOW (0x08000000)
        // Note: CREATE_BREAKAWAY_FROM_JOB (0x01000000) is NOT used because it requires
        // the parent to be in a job that allows breakaway, which fails under test harnesses
        // and modern Windows implicit job objects with "Access Denied" (OS error 5).
        cmd.creation_flags(0x00000200 | 0x08000000);
    }

    let child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn shim: {}", e))?;
    let shim_pid = child.id();

    daemon_log!(
        "Spawned pty-shim for session {}: pid={}, pipe={}",
        session_id,
        shim_pid,
        pipe_name
    );

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(ShimMetadata {
        session_id: session_id.to_string(),
        shim_pid,
        shim_pipe_name: pipe_name,
        shell_pid: 0, // Updated when we connect and get StatusInfo
        shell_type: shell_type.clone(),
        cwd: cwd.map(|s| s.to_string()),
        rows,
        cols,
        created_at: now,
    })
}

/// Connect to a running pty-shim's named pipe.
/// Retries a few times since the shim may still be starting up.
#[cfg(windows)]
pub fn connect_to_shim(pipe_name: &str) -> Result<std::fs::File, String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

    let wide: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Retry loop -- shim may not have created the pipe yet
    for attempt in 0..30 {
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

        if handle != INVALID_HANDLE_VALUE {
            daemon_log!(
                "Connected to shim pipe {} (attempt {})",
                pipe_name,
                attempt + 1
            );
            return Ok(unsafe { std::fs::File::from_raw_handle(handle as _) });
        }

        if attempt < 29 {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    Err(format!(
        "Failed to connect to shim pipe {} after 30 attempts",
        pipe_name
    ))
}

#[cfg(not(windows))]
pub fn connect_to_shim(_pipe_name: &str) -> Result<std::fs::File, String> {
    Err("Named pipes only supported on Windows".to_string())
}

/// Duplicate a file handle so we can have separate reader/writer handles.
/// On Windows named pipes, a single handle can be used for both read and write,
/// but using separate handles avoids serialization issues between the reader
/// thread and writer calls.
#[cfg(windows)]
pub fn duplicate_handle(file: &std::fs::File) -> io::Result<std::fs::File> {
    use std::os::windows::io::{AsRawHandle, FromRawHandle};
    use winapi::um::handleapi::DuplicateHandle;
    use winapi::um::processthreadsapi::GetCurrentProcess;

    let mut new_handle = std::ptr::null_mut();
    let result = unsafe {
        DuplicateHandle(
            GetCurrentProcess(),
            file.as_raw_handle() as _,
            GetCurrentProcess(),
            &mut new_handle,
            0,
            0,
            winapi::um::winnt::DUPLICATE_SAME_ACCESS,
        )
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { std::fs::File::from_raw_handle(new_handle as _) })
}

#[cfg(not(windows))]
pub fn duplicate_handle(_file: &std::fs::File) -> io::Result<std::fs::File> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "DuplicateHandle only available on Windows",
    ))
}

/// Check if a process is still alive by its PID.
#[cfg(windows)]
pub fn is_process_alive(pid: u32) -> bool {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::OpenProcess;
    use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        // Check if the process has exited
        let mut exit_code: u32 = 0;
        let result = winapi::um::processthreadsapi::GetExitCodeProcess(
            handle,
            &mut exit_code as *mut u32 as *mut _,
        );
        CloseHandle(handle);
        // STILL_ACTIVE = 259
        result != 0 && exit_code == 259
    }
}

#[cfg(not(windows))]
pub fn is_process_alive(_pid: u32) -> bool {
    false
}

/// Find the pty-shim binary. Looks next to the daemon binary first,
/// then falls back to common locations.
fn find_shim_binary() -> Result<std::path::PathBuf, String> {
    // First: look next to the current executable
    if let Ok(exe) = std::env::current_exe() {
        let dir = exe.parent().unwrap_or_else(|| std::path::Path::new("."));
        let shim = dir.join("godly-pty-shim.exe");
        if shim.exists() {
            return Ok(shim);
        }
        // Also check without .exe (for cross-platform)
        let shim = dir.join("godly-pty-shim");
        if shim.exists() {
            return Ok(shim);
        }
    }

    // Fallback: check target/debug and target/release
    for profile in &["debug", "release"] {
        let path = std::path::PathBuf::from(format!("target/{}/godly-pty-shim.exe", profile));
        if path.exists() {
            return Ok(path);
        }
    }

    Err("Could not find godly-pty-shim binary".to_string())
}

/// Convert ShellType to the string format the shim expects.
fn shell_type_to_shim_arg(shell_type: &ShellType) -> String {
    match shell_type {
        ShellType::Windows => "windows".to_string(),
        ShellType::Pwsh => "pwsh".to_string(),
        ShellType::Cmd => "cmd".to_string(),
        ShellType::Wsl { distribution } => match distribution {
            Some(d) => format!("wsl:{}", d),
            None => "wsl".to_string(),
        },
        ShellType::Custom { program, args } => match args {
            Some(a) => format!("{}:{}", program, a.join(" ")),
            None => program.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_type_to_shim_arg_windows() {
        assert_eq!(shell_type_to_shim_arg(&ShellType::Windows), "windows");
    }

    #[test]
    fn test_shell_type_to_shim_arg_pwsh() {
        assert_eq!(shell_type_to_shim_arg(&ShellType::Pwsh), "pwsh");
    }

    #[test]
    fn test_shell_type_to_shim_arg_cmd() {
        assert_eq!(shell_type_to_shim_arg(&ShellType::Cmd), "cmd");
    }

    #[test]
    fn test_shell_type_to_shim_arg_wsl_default() {
        assert_eq!(
            shell_type_to_shim_arg(&ShellType::Wsl {
                distribution: None
            }),
            "wsl"
        );
    }

    #[test]
    fn test_shell_type_to_shim_arg_wsl_distro() {
        assert_eq!(
            shell_type_to_shim_arg(&ShellType::Wsl {
                distribution: Some("Ubuntu".to_string())
            }),
            "wsl:Ubuntu"
        );
    }

    #[test]
    fn test_shell_type_to_shim_arg_custom_no_args() {
        assert_eq!(
            shell_type_to_shim_arg(&ShellType::Custom {
                program: "nu.exe".to_string(),
                args: None
            }),
            "nu.exe"
        );
    }

    #[test]
    fn test_shell_type_to_shim_arg_custom_with_args() {
        assert_eq!(
            shell_type_to_shim_arg(&ShellType::Custom {
                program: "fish".to_string(),
                args: Some(vec!["-l".to_string(), "--init".to_string()])
            }),
            "fish:-l --init"
        );
    }
}
