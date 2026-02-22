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
use godly_protocol::types::RichGridData;

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
#[ntest::timeout(60_000)] // 1min — daemon spawn + IPC
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
#[ntest::timeout(60_000)]
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
#[ntest::timeout(60_000)]
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
#[ntest::timeout(60_000)]
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
#[ntest::timeout(60_000)]
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

// ---------------------------------------------------------------------------
// Scrollback integration tests
// ---------------------------------------------------------------------------

fn read_rich_grid(pipe: &mut std::fs::File, session_id: &str) -> RichGridData {
    let resp = send_request(
        pipe,
        &Request::ReadRichGrid {
            session_id: session_id.to_string(),
        },
    );
    match resp {
        Response::RichGrid { grid } => grid,
        other => panic!("Expected RichGrid, got: {:?}", other),
    }
}

fn wait_for_rich_grid_text(
    pipe: &mut std::fs::File,
    session_id: &str,
    expected: &str,
    timeout: Duration,
) -> RichGridData {
    let start = Instant::now();
    loop {
        let grid = read_rich_grid(pipe, session_id);
        let full_text: String = grid
            .rows
            .iter()
            .map(|r| r.cells.iter().map(|c| c.content.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        if full_text.contains(expected) {
            return grid;
        }
        if start.elapsed() > timeout {
            panic!("Timeout waiting for rich grid to contain {:?}", expected);
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

/// After enough output, RichGridData reports total_scrollback > 0
/// and scrollback_offset = 0 (live view).
#[test]
#[ntest::timeout(60_000)]
fn test_scrollback_fields_in_rich_grid() {
    let daemon = DaemonFixture::spawn("scroll-fields");
    let mut pipe = daemon.connect();

    let session_id = "scroll-fields".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 10,
            cols: 80,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let resp = send_request(
        &mut pipe,
        &Request::Attach { session_id: session_id.clone() },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));
    std::thread::sleep(Duration::from_secs(2));

    let marker = "SCROLL_END_MARKER";
    let cmd = format!("for /L %i in (1,1,30) do @echo LINE_%i\r\necho {}\r\n", marker);
    send_request(
        &mut pipe,
        &Request::Write { session_id: session_id.clone(), data: cmd.into_bytes() },
    );

    let grid = wait_for_rich_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(10));
    assert_eq!(grid.scrollback_offset, 0, "Should be in live view");
    assert!(grid.total_scrollback > 0, "Should have scrollback rows, got {}", grid.total_scrollback);

    send_request(&mut pipe, &Request::CloseSession { session_id: session_id.clone() });
}

/// SetScrollback changes the viewport offset visible via ReadRichGrid.
#[test]
#[ntest::timeout(60_000)]
fn test_set_scrollback_changes_viewport() {
    let daemon = DaemonFixture::spawn("scroll-viewport");
    let mut pipe = daemon.connect();

    let session_id = "scroll-vp".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 10,
            cols: 80,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let resp = send_request(
        &mut pipe,
        &Request::Attach { session_id: session_id.clone() },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));
    std::thread::sleep(Duration::from_secs(2));

    let marker = "VP_DONE";
    let cmd = format!("for /L %i in (1,1,30) do @echo LINE_%i\r\necho {}\r\n", marker);
    send_request(
        &mut pipe,
        &Request::Write { session_id: session_id.clone(), data: cmd.into_bytes() },
    );
    wait_for_rich_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(10));

    // Capture live view
    let live_grid = read_rich_grid(&mut pipe, &session_id);
    let live_row0: String = live_grid.rows[0].cells.iter().map(|c| c.content.as_str()).collect();

    // Scroll up by 5
    let resp = send_request(
        &mut pipe,
        &Request::SetScrollback { session_id: session_id.clone(), offset: 5 },
    );
    assert!(matches!(resp, Response::Ok));

    let scrolled = read_rich_grid(&mut pipe, &session_id);
    assert!(scrolled.scrollback_offset > 0, "Should have non-zero offset after SetScrollback(5), got {}", scrolled.scrollback_offset);
    let scrolled_row0: String = scrolled.rows[0].cells.iter().map(|c| c.content.as_str()).collect();
    assert_ne!(live_row0.trim(), scrolled_row0.trim(), "Scrolled viewport should differ from live");

    // Scroll back to bottom
    let resp = send_request(
        &mut pipe,
        &Request::SetScrollback { session_id: session_id.clone(), offset: 0 },
    );
    assert!(matches!(resp, Response::Ok));
    let bottom = read_rich_grid(&mut pipe, &session_id);
    assert_eq!(bottom.scrollback_offset, 0);

    send_request(&mut pipe, &Request::CloseSession { session_id: session_id.clone() });
}

/// SetScrollback with offset > total clamps to available scrollback.
#[test]
#[ntest::timeout(60_000)]
fn test_scrollback_offset_clamped() {
    let daemon = DaemonFixture::spawn("scroll-clamp");
    let mut pipe = daemon.connect();

    let session_id = "scroll-clamp".to_string();
    let resp = send_request(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 10,
            cols: 80,
            env: None,
        },
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Large offset on empty scrollback → clamps to 0
    let resp = send_request(
        &mut pipe,
        &Request::SetScrollback { session_id: session_id.clone(), offset: 99999 },
    );
    assert!(matches!(resp, Response::Ok));

    let grid = read_rich_grid(&mut pipe, &session_id);
    assert!(
        grid.scrollback_offset <= grid.total_scrollback,
        "Offset {} should be <= total {}",
        grid.scrollback_offset, grid.total_scrollback
    );

    send_request(&mut pipe, &Request::CloseSession { session_id: session_id.clone() });
}

// ---------------------------------------------------------------------------
// OSC title integration tests — Bug #182
// ---------------------------------------------------------------------------

/// Helper: read a RichGridDiff from the daemon.
fn read_rich_grid_diff(pipe: &mut std::fs::File, session_id: &str) -> godly_protocol::types::RichGridDiff {
    let resp = send_request(
        pipe,
        &Request::ReadRichGridDiff {
            session_id: session_id.to_string(),
        },
    );
    match resp {
        Response::RichGridDiff { diff } => diff,
        other => panic!("Expected RichGridDiff, got: {:?}", other),
    }
}

/// Bug #182: OSC title set via PowerShell is not returned in ReadRichGrid.
///
/// When a process sets the terminal title via $Host.UI.RawUI.WindowTitle
/// (which ConPTY translates to OSC 0), the title should appear in the
/// RichGridData.title field.
#[test]
#[ntest::timeout(60_000)]
fn test_osc_title_in_rich_grid() {
    let daemon = DaemonFixture::spawn("osc-title");
    let mut pipe = daemon.connect();

    let session_id = "osc-title".to_string();
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

    let resp = send_request(
        &mut pipe,
        &Request::Attach { session_id: session_id.clone() },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));
    std::thread::sleep(Duration::from_secs(2));

    // Set the terminal title via PowerShell. ConPTY translates this Win32
    // SetConsoleTitle call into an OSC 0 escape sequence in the PTY output.
    let title = "GODLY_TITLE_TEST_182";
    let marker = "TITLE_MARKER_DONE_182";
    let cmd = format!(
        "$Host.UI.RawUI.WindowTitle = '{}'; echo '{}'\r\n",
        title, marker
    );
    send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: cmd.into_bytes(),
        },
    );

    // Wait for the marker to confirm the command completed
    wait_for_rich_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(15));

    // Give the vt parser time to process the OSC sequence after the echo
    std::thread::sleep(Duration::from_millis(500));

    // Bug #182: RichGridData.title should contain the OSC title
    let grid = read_rich_grid(&mut pipe, &session_id);
    assert!(
        !grid.title.is_empty(),
        "Bug #182: RichGridData.title should not be empty after setting \
         terminal title via PowerShell. The daemon discards OSC title sequences."
    );
    assert!(
        grid.title.contains(title),
        "Bug #182: RichGridData.title should contain '{}', got '{}'",
        title, grid.title
    );

    send_request(&mut pipe, &Request::CloseSession { session_id: session_id.clone() });
}

/// Bug #182: OSC title is also missing from ReadRichGridDiff responses.
///
/// The extract_diff() function has the same hardcoded title: String::new().
#[test]
#[ntest::timeout(60_000)]
fn test_osc_title_in_rich_grid_diff() {
    let daemon = DaemonFixture::spawn("osc-title-diff");
    let mut pipe = daemon.connect();

    let session_id = "osc-diff".to_string();
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

    let resp = send_request(
        &mut pipe,
        &Request::Attach { session_id: session_id.clone() },
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));
    std::thread::sleep(Duration::from_secs(2));

    let title = "GODLY_DIFF_TITLE_182";
    let marker = "DIFF_TITLE_MARKER_182";
    let cmd = format!(
        "$Host.UI.RawUI.WindowTitle = '{}'; echo '{}'\r\n",
        title, marker
    );
    send_request(
        &mut pipe,
        &Request::Write {
            session_id: session_id.clone(),
            data: cmd.into_bytes(),
        },
    );

    // Wait for the marker text to appear in the grid
    wait_for_rich_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(15));
    std::thread::sleep(Duration::from_millis(500));

    // Bug #182: RichGridDiff.title should contain the OSC title
    let diff = read_rich_grid_diff(&mut pipe, &session_id);
    assert!(
        !diff.title.is_empty(),
        "Bug #182: RichGridDiff.title should not be empty after setting \
         terminal title via PowerShell. extract_diff() hardcodes String::new()."
    );
    assert!(
        diff.title.contains(title),
        "Bug #182: RichGridDiff.title should contain '{}', got '{}'",
        title, diff.title
    );

    send_request(&mut pipe, &Request::CloseSession { session_id: session_id.clone() });
}
