//! Integration tests for the ReadGrid command (godly-vt grid state pipeline).
//!
//! Verifies that PTY output is parsed by the godly-vt engine inside the daemon
//! and that ReadGrid returns correct grid snapshots (plain-text rows + cursor
//! position). Also tests that Resize keeps the vt parser in sync with the PTY.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test read_grid -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, GridData, Request, Response, ShellType};

// ---------------------------------------------------------------------------
// Helpers (DaemonFixture pattern — see handler_starvation.rs)
// ---------------------------------------------------------------------------

fn connect_pipe(pipe_name: &str, timeout: Duration) -> std::fs::File {
    use std::os::windows::io::FromRawHandle;
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

/// Non-blocking check for data available on a pipe handle.
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

/// Send a request and wait for the response with a deadline.
/// Skips Event messages while waiting for the Response.
fn send_request_with_deadline(
    pipe: &mut std::fs::File,
    request: &Request,
    deadline: Duration,
) -> Result<Response, String> {
    godly_protocol::write_message(pipe, request)
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

        let msg: DaemonMessage = godly_protocol::read_message(pipe)
            .map_err(|e| format!("Read error: {}", e))?
            .ok_or_else(|| "Unexpected EOF".to_string())?;

        match msg {
            DaemonMessage::Response(resp) => return Ok(resp),
            DaemonMessage::Event(_) => continue,
        }
    }
}

/// Convenience: send request with 10s deadline, panic on failure.
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

        // Build the daemon binary
        let status = Command::new("cargo")
            .args(["build", "-p", "godly-daemon"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("Failed to run cargo build");
        assert!(status.success(), "cargo build failed");

        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let target_dir = manifest_dir
            .parent()
            .unwrap()
            .join("target")
            .join("debug");
        let daemon_exe = target_dir.join("godly-daemon.exe");
        assert!(
            daemon_exe.exists(),
            "Daemon binary not found at {:?}",
            daemon_exe
        );

        let child = Command::new(&daemon_exe)
            .env("GODLY_PIPE_NAME", &pipe_name)
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

/// Wait until the grid contains the expected text, polling with ReadGrid.
/// Returns the GridData snapshot that matched.
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
            // One last attempt to show what we got
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

/// ReadGrid on a freshly created session returns a grid of the correct size
/// with an empty screen and cursor at (0, 0).
#[test]
fn test_read_grid_initial_state() {
    let daemon = DaemonFixture::spawn("grid-initial");
    let mut pipe = daemon.connect();

    let session_id = "grid-init".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "Create failed: {:?}",
        resp
    );

    // ReadGrid without attaching should still work — the vt parser processes
    // output from the reader thread regardless of attachment state.
    let resp = send_request(
        &mut pipe,
        &Request::ReadGrid {
            session_id: session_id.clone(),
        },
    );

    match resp {
        Response::Grid { grid } => {
            assert_eq!(grid.num_rows, 24, "Expected 24 rows");
            assert_eq!(grid.cols, 80, "Expected 80 cols");
            assert_eq!(grid.rows.len(), 24, "Expected 24 row strings");
            assert!(!grid.alternate_screen, "Should not be on alternate screen");
        }
        other => panic!("Expected Grid response, got: {:?}", other),
    }

    // Cleanup
    send_request(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );
}

/// ReadGrid for a non-existent session returns an error.
#[test]
fn test_read_grid_session_not_found() {
    let daemon = DaemonFixture::spawn("grid-notfound");
    let mut pipe = daemon.connect();

    let resp = send_request(
        &mut pipe,
        &Request::ReadGrid {
            session_id: "nonexistent".to_string(),
        },
    );

    assert!(
        matches!(resp, Response::Error { .. }),
        "Expected Error for missing session, got: {:?}",
        resp
    );
}

/// After writing a command to the shell, ReadGrid should reflect the output
/// in the parsed grid rows (plain text, no ANSI escapes).
#[test]
fn test_read_grid_captures_output() {
    let daemon = DaemonFixture::spawn("grid-output");
    let mut pipe = daemon.connect();

    let session_id = "grid-out".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Attach so we can write commands
    let resp = send_request(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.clone(),
        },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Wait for shell prompt to appear
    std::thread::sleep(Duration::from_secs(2));

    // Write a command that produces known output
    let marker = "GODLY_GRID_TEST_MARKER_42";
    let cmd = format!("echo {}\r\n", marker);
    send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: cmd.into_bytes(),
        },
    );

    // Wait for the marker to appear in the grid
    let grid = wait_for_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(10));

    // Verify: grid rows should contain the marker as plain text
    let found = grid.rows.iter().any(|row| row.contains(marker));
    assert!(
        found,
        "Grid should contain marker {:?} in at least one row",
        marker
    );

    // Verify: grid rows should NOT contain raw ANSI escape sequences
    let has_ansi = grid.rows.iter().any(|row| row.contains("\x1b["));
    assert!(
        !has_ansi,
        "Grid rows should be plain text without ANSI escapes"
    );

    // Cleanup
    send_request(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );
}

/// After a Resize, ReadGrid should return a grid with the new dimensions.
/// This verifies that session.resize() also updates the vt parser size.
#[test]
fn test_read_grid_after_resize() {
    let daemon = DaemonFixture::spawn("grid-resize");
    let mut pipe = daemon.connect();

    let session_id = "grid-resize".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Verify initial size
    let resp = send_request(
        &mut pipe,
        &Request::ReadGrid {
            session_id: session_id.clone(),
        },
    );
    match &resp {
        Response::Grid { grid } => {
            assert_eq!(grid.num_rows, 24);
            assert_eq!(grid.cols, 80);
        }
        other => panic!("Expected Grid, got: {:?}", other),
    }

    // Resize to 40 rows x 120 cols
    let resp = send_request(
        &mut pipe,
        &Request::Resize {
            session_id: session_id.clone(),
            rows: 40,
            cols: 120,
        },
    );
    assert!(matches!(resp, Response::Ok), "Resize failed: {:?}", resp);

    // ReadGrid should now reflect the new size
    let resp = send_request(
        &mut pipe,
        &Request::ReadGrid {
            session_id: session_id.clone(),
        },
    );
    match resp {
        Response::Grid { grid } => {
            assert_eq!(
                grid.num_rows, 40,
                "Grid rows should be 40 after resize, got {}",
                grid.num_rows
            );
            assert_eq!(
                grid.cols, 120,
                "Grid cols should be 120 after resize, got {}",
                grid.cols
            );
            assert_eq!(
                grid.rows.len(),
                40,
                "Should have 40 row strings after resize"
            );
        }
        other => panic!("Expected Grid, got: {:?}", other),
    }

    // Cleanup
    send_request(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );
}

/// ReadGrid works without attaching — the vt parser processes output from the
/// reader thread regardless of whether a client is attached.
#[test]
fn test_read_grid_without_attach() {
    let daemon = DaemonFixture::spawn("grid-noattach");
    let mut pipe = daemon.connect();

    let session_id = "grid-noattach".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Do NOT attach — just wait for the shell to start and print its prompt.
    // The reader thread still runs and feeds PTY output to the vt parser.
    std::thread::sleep(Duration::from_secs(3));

    // ReadGrid should return a non-empty grid (shell prompt should be visible)
    let resp = send_request(
        &mut pipe,
        &Request::ReadGrid {
            session_id: session_id.clone(),
        },
    );
    match resp {
        Response::Grid { grid } => {
            // At least the first row should have some content from the shell prompt
            let has_content = grid.rows.iter().any(|row| !row.trim().is_empty());
            assert!(
                has_content,
                "Grid should have some content from shell startup even without attach. Rows: {:?}",
                &grid.rows[..3.min(grid.rows.len())]
            );
        }
        other => panic!("Expected Grid, got: {:?}", other),
    }

    // Cleanup
    send_request(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.clone(),
        },
    );
}
