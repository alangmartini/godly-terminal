//! Bug #669: New terminal in workspace does not inherit workspace CWD.
//!
//! Tests that the daemon correctly sets the CWD when creating a session,
//! and that PowerShell profiles do not override the CWD.
//!
//! Run with:
//!   cd src-tauri && cargo nextest run -p godly-daemon --test workspace_cwd

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, GridData, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// Helpers (DaemonFixture pattern — mirrors read_grid.rs)
// ---------------------------------------------------------------------------

fn connect_pipe(pipe_name: &str, timeout: Duration) -> std::fs::File {
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let start = Instant::now();
    loop {
        let handle = unsafe {
            CreateFileW(
                wide_name.as_ptr(),
                GENERIC_READ | GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                std::ptr::null_mut(),
                OPEN_EXISTING,
                0,
                std::ptr::null_mut(),
            )
        };

        if handle != INVALID_HANDLE_VALUE {
            return unsafe { std::fs::File::from_raw_handle(handle as _) };
        }

        if start.elapsed() > timeout {
            let err = unsafe { GetLastError() };
            panic!(
                "Failed to connect to pipe '{}' within {:?} (error: {})",
                pipe_name, timeout, err
            );
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn pipe_has_data(pipe: &std::fs::File) -> bool {
    use winapi::um::namedpipeapi::PeekNamedPipe;

    let handle = pipe.as_raw_handle();
    let mut bytes_available: u32 = 0;
    let result = unsafe {
        PeekNamedPipe(
            handle as *mut _,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut bytes_available,
            std::ptr::null_mut(),
        )
    };
    result != 0 && bytes_available > 0
}

fn send_request_with_deadline(
    pipe: &mut std::fs::File,
    request: &Request,
    deadline: Duration,
) -> Result<Response, String> {
    godly_protocol::write_request(pipe, request)
        .map_err(|e| format!("Failed to write request: {}", e))?;

    let start = Instant::now();
    loop {
        if start.elapsed() > deadline {
            return Err(format!("Deadline exceeded ({:?})", deadline));
        }

        if !pipe_has_data(pipe) {
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }

        let msg: DaemonMessage = godly_protocol::read_daemon_message(pipe)
            .map_err(|e| format!("Read error: {}", e))?
            .ok_or_else(|| "Unexpected EOF".to_string())?;

        match msg {
            DaemonMessage::Response(resp) => return Ok(resp),
            DaemonMessage::Event(_) => continue,
        }
    }
}

fn send_request(pipe: &mut std::fs::File, request: &Request) -> Response {
    send_request_with_deadline(pipe, request, Duration::from_secs(10))
        .unwrap_or_else(|e| panic!("Request failed: {}", e))
}

struct DaemonFixture {
    child: Child,
    pipe_name: String,
}

impl DaemonFixture {
    fn spawn(test_name: &str) -> Self {
        let pipe_name = format!(
            r"\\.\pipe\godly-test-{}-{}",
            test_name,
            std::process::id()
        );

        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let target_dir = manifest_dir
            .parent()
            .unwrap()
            .join("target")
            .join("debug");
        let daemon_exe = target_dir.join("godly-daemon.exe");
        assert!(
            daemon_exe.exists(),
            "Daemon binary not found at {:?}. Run `cargo build -p godly-daemon` first.",
            daemon_exe
        );

        let child = Command::new(&daemon_exe)
            .env("GODLY_PIPE_NAME", &pipe_name)
            .env("GODLY_INSTANCE", pipe_name.trim_start_matches(r"\\.\pipe\"))
            .env("GODLY_NO_DETACH", "1")
            .stderr(std::process::Stdio::piped())
            .spawn()
            .expect("Failed to spawn daemon");

        std::thread::sleep(Duration::from_millis(500));

        Self { child, pipe_name }
    }

    fn connect(&self) -> std::fs::File {
        connect_pipe(&self.pipe_name, Duration::from_secs(5))
    }
}

impl Drop for DaemonFixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn wait_for_grid_text(
    pipe: &mut std::fs::File,
    session_id: &str,
    expected: &str,
    timeout: Duration,
) -> GridData {
    let start = Instant::now();
    loop {
        let resp = send_request(
            pipe,
            &Request::ReadGrid {
                session_id: session_id.to_string(),
            },
        );
        match resp {
            Response::Grid { grid } => {
                let full_text: String = grid.rows.join("\n");
                if full_text.contains(expected) {
                    return grid;
                }
            }
            other => panic!("Expected Grid response, got: {:?}", other),
        }

        if start.elapsed() > timeout {
            let resp = send_request(
                pipe,
                &Request::ReadGrid {
                    session_id: session_id.to_string(),
                },
            );
            if let Response::Grid { grid } = resp {
                panic!(
                    "Timeout waiting for grid to contain {:?}. Grid rows:\n{}",
                    expected,
                    grid.rows
                        .iter()
                        .enumerate()
                        .map(|(i, r)| format!("  [{}] {:?}", i, r))
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            panic!("Timeout waiting for grid to contain {:?}", expected);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug #669: When a session is created with cwd=Some("C:\Windows\Temp"),
/// the spawned PowerShell shell should start in that directory.
///
/// This tests the daemon + PTY shim CWD handling. PowerShell is spawned
/// with `-NoLogo` but NOT `-NoProfile`, so the user's profile can
/// potentially override the CWD via Set-Location.
///
/// If this test FAILS, it means either:
/// 1. The daemon doesn't pass CWD to the PTY shim correctly, or
/// 2. The PowerShell profile overrides the CWD after spawn.
#[test]
#[ntest::timeout(60_000)]
fn test_session_cwd_powershell() {
    let target_dir = r"C:\Windows\Temp";
    let daemon = DaemonFixture::spawn("cwd-ps");
    let mut pipe = daemon.connect();

    let session_id = "cwd-ps".to_string();
    // Bug #669: Create session with explicit CWD
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: Some(target_dir.to_string()),
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "CreateSession failed: {:?}",
        resp
    );

    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.clone(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Wait for PowerShell to fully start (including profile execution)
    std::thread::sleep(Duration::from_secs(3));

    // Query the current directory
    let marker = "CWD_MARKER_669";
    let cmd = format!(
        "Write-Host '{}' (Get-Location).Path\r\n",
        marker
    );
    send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: cmd.into_bytes(),
        },
    );

    // Wait for the marker to appear in the grid
    let grid = wait_for_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(10));

    // Find the row containing the marker and extract the CWD
    let marker_row = grid
        .rows
        .iter()
        .find(|row| row.contains(marker))
        .unwrap_or_else(|| {
            panic!(
                "Marker {:?} not found in grid. Rows:\n{}",
                marker,
                grid.rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| format!("  [{}] {:?}", i, r))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        });

    // Bug #669: The CWD should be the target directory, not %USERPROFILE%
    // or whatever the PowerShell profile sets it to.
    assert!(
        marker_row.to_lowercase().contains(&target_dir.to_lowercase()),
        "Bug #669: CWD should be '{}' but the grid row shows: {:?}\n\
         This may indicate the PowerShell profile overrides the CWD set at spawn time.\n\
         Full grid:\n{}",
        target_dir,
        marker_row,
        grid.rows
            .iter()
            .enumerate()
            .map(|(i, r)| format!("  [{}] {:?}", i, r))
            .collect::<Vec<_>>()
            .join("\n")
    );

    send_request(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );
}

/// Bug #669: Baseline CWD test using cmd.exe (no profile interference).
///
/// Verifies the daemon + PTY shim correctly passes CWD to the shell
/// at the OS level via CreateProcessW's lpCurrentDirectory parameter.
/// Uses cmd.exe which has no profile mechanism, so this isolates the
/// daemon's CWD handling from any shell-specific behavior.
#[test]
#[ntest::timeout(60_000)]
fn test_session_cwd_cmd() {
    let target_dir = r"C:\Windows\Temp";
    let daemon = DaemonFixture::spawn("cwd-cmd");
    let mut pipe = daemon.connect();

    let session_id = "cwd-cmd".to_string();
    // Bug #669: Create session with explicit CWD using cmd.exe
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Cmd,
            cwd: Some(target_dir.to_string()),
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "CreateSession failed: {:?}",
        resp
    );

    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.clone(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    std::thread::sleep(Duration::from_secs(2));

    // In cmd.exe, `cd` with no arguments prints the current directory
    let marker = "CWD_CMD_MARKER_669";
    let cmd = format!("echo {} & cd\r\n", marker);
    send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: cmd.into_bytes(),
        },
    );

    let grid = wait_for_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(10));

    // Find the line AFTER the marker line that contains just the directory path.
    // In cmd.exe, `cd` outputs the directory on a separate line.
    let grid_text = grid.rows.join("\n").to_lowercase();
    assert!(
        grid_text.contains(&target_dir.to_lowercase()),
        "Bug #669: CWD should be '{}' but not found in grid output.\n\
         The daemon may not be passing CWD to the PTY shim correctly.\n\
         Grid:\n{}",
        target_dir,
        grid.rows
            .iter()
            .enumerate()
            .map(|(i, r)| format!("  [{}] {:?}", i, r))
            .collect::<Vec<_>>()
            .join("\n")
    );

    send_request(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );
}

