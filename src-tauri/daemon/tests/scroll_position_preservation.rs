//! Bug #202: Scroll position not preserved — terminal snaps to bottom on new output.
//!
//! When the user scrolls up (SetScrollback > 0) and new output arrives from the
//! PTY, the viewport offset must be preserved (and incremented to track the same
//! content). These tests verify that the daemon's godly-vt parser does NOT reset
//! scrollback_offset to 0 when new lines scroll in.
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test scroll_position_preservation -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Request, Response, ShellType};
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
        assert!(daemon_exe.exists(), "Daemon binary not found at {:?}", daemon_exe);

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
            panic!(
                "Timeout waiting for rich grid to contain {:?}. Got:\n{}",
                expected, full_text
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn grid_text(grid: &RichGridData) -> String {
    grid.rows
        .iter()
        .map(|r| r.cells.iter().map(|c| c.content.as_str()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Tests — Bug #202: Scroll position snaps to bottom on new output
// ---------------------------------------------------------------------------

/// Bug #202: After SetScrollback(N), writing new output to the PTY should NOT
/// reset the scrollback offset to 0. The daemon's godly-vt parser must preserve
/// (and increment) the offset so the viewport tracks the same content.
#[test]
#[ntest::timeout(60_000)]
fn test_scrollback_preserved_during_new_output() {
    let daemon = DaemonFixture::spawn("scroll-preserve");
    let mut pipe = daemon.connect();

    let session_id = "scroll-preserve".to_string();
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

    // Phase 1: Generate enough output to have scrollback (30 numbered lines in a 10-row terminal)
    let marker1 = "PHASE1_DONE_202";
    let cmd1 = format!("for /L %i in (1,1,30) do @echo LINE_%i\r\necho {}\r\n", marker1);
    send_request(
        &mut pipe,
        &Request::Write { session_id: session_id.clone(), data: cmd1.into_bytes() },
    );
    wait_for_rich_grid_text(&mut pipe, &session_id, marker1, Duration::from_secs(10));

    // Phase 2: Scroll up by 10 rows
    let resp = send_request(
        &mut pipe,
        &Request::SetScrollback { session_id: session_id.clone(), offset: 10 },
    );
    assert!(matches!(resp, Response::Ok));

    // Verify the viewport shifted into scrollback
    let scrolled_grid = read_rich_grid(&mut pipe, &session_id);
    assert!(
        scrolled_grid.scrollback_offset >= 10,
        "Bug #202: Expected scrollback_offset >= 10 after SetScrollback(10), got {}",
        scrolled_grid.scrollback_offset
    );
    // Phase 3: Write MORE output while scrolled up (simulates new shell output)
    let marker2 = "PHASE2_DONE_202";
    let cmd2 = format!("for /L %i in (31,1,40) do @echo NEWLINE_%i\r\necho {}\r\n", marker2);
    send_request(
        &mut pipe,
        &Request::Write { session_id: session_id.clone(), data: cmd2.into_bytes() },
    );

    // Wait for the new output to be processed by polling grid text at offset 0 (live view).
    // We set scrollback to 0 on a SEPARATE connection to avoid disturbing the scrolled viewport.
    // Actually, we just need to wait for the output to arrive — we'll poll ReadRichGrid and check
    // if the new lines increased total_scrollback.
    let start = Instant::now();
    loop {
        let grid = read_rich_grid(&mut pipe, &session_id);
        if grid.total_scrollback > scrolled_grid.total_scrollback + 5 {
            break;
        }
        if start.elapsed() > Duration::from_secs(15) {
            panic!(
                "Timeout waiting for new output to produce scrollback. \
                 total_scrollback before={} after={}",
                scrolled_grid.total_scrollback, grid.total_scrollback
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    // Phase 4: Read the grid — scrollback_offset should be PRESERVED (not reset to 0)
    let after_output = read_rich_grid(&mut pipe, &session_id);

    // Bug #202: scrollback_offset must be >= 10. The scroll_up() logic in godly-vt
    // should have incremented it as new lines arrived, keeping the viewport on the
    // same content. If it's 0, the parser reset to live view on new output.
    assert!(
        after_output.scrollback_offset >= 10,
        "Bug #202: scrollback_offset reset to {} after new output arrived (expected >= 10). \
         The daemon snapped the viewport to the bottom instead of preserving the user's scroll position.",
        after_output.scrollback_offset
    );

    // Additionally verify the viewport content is NOT the latest output.
    // If the viewport stayed scrolled, it should NOT contain NEWLINE_40.
    let after_text = grid_text(&after_output);
    assert!(
        !after_text.contains("NEWLINE_40"),
        "Bug #202: Viewport shows latest output (NEWLINE_40) while user is scrolled up. \
         The daemon should show older scrollback content.\nViewport:\n{}",
        after_text
    );

    send_request(&mut pipe, &Request::CloseSession { session_id: session_id.clone() });
}

/// Bug #202: Rapid SetScrollback + Write interleaving should not lose the scroll position.
/// Simulates the race between frontend setScrollback IPC and concurrent PTY output.
#[test]
#[ntest::timeout(60_000)]
fn test_scrollback_stable_under_concurrent_output() {
    let daemon = DaemonFixture::spawn("scroll-concurrent");
    let mut pipe = daemon.connect();

    let session_id = "scroll-conc".to_string();
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

    // Generate initial scrollback
    let marker = "INIT_DONE_CONC";
    let cmd = format!("for /L %i in (1,1,50) do @echo LINE_%i\r\necho {}\r\n", marker);
    send_request(
        &mut pipe,
        &Request::Write { session_id: session_id.clone(), data: cmd.into_bytes() },
    );
    wait_for_rich_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(10));

    // Scroll up
    let resp = send_request(
        &mut pipe,
        &Request::SetScrollback { session_id: session_id.clone(), offset: 20 },
    );
    assert!(matches!(resp, Response::Ok));

    // Immediately start generating more output while scrolled up
    let marker2 = "BURST_DONE_CONC";
    let cmd2 = format!("for /L %i in (1,1,20) do @echo BURST_%i\r\necho {}\r\n", marker2);
    send_request(
        &mut pipe,
        &Request::Write { session_id: session_id.clone(), data: cmd2.into_bytes() },
    );

    // Wait for burst output to complete by polling total_scrollback growth
    let start = Instant::now();
    let initial_grid = read_rich_grid(&mut pipe, &session_id);
    loop {
        let grid = read_rich_grid(&mut pipe, &session_id);
        if grid.total_scrollback > initial_grid.total_scrollback + 10 {
            break;
        }
        if start.elapsed() > Duration::from_secs(15) {
            break; // Proceed to assertion even if not all output arrived
        }
        std::thread::sleep(Duration::from_millis(100));
    }

    // Read grid multiple times to check stability
    let mut offsets = Vec::new();
    for _ in 0..5 {
        let grid = read_rich_grid(&mut pipe, &session_id);
        offsets.push(grid.scrollback_offset);
        std::thread::sleep(Duration::from_millis(100));
    }

    // Bug #202: ALL reads should show offset >= 20 (never reset to 0)
    for (i, &offset) in offsets.iter().enumerate() {
        assert!(
            offset >= 20,
            "Bug #202: Read #{} returned scrollback_offset={} (expected >= 20). \
             The viewport snapped to bottom during concurrent output.",
            i + 1, offset
        );
    }

    // Offsets should be stable (not jumping around)
    let min_offset = *offsets.iter().min().unwrap();
    let max_offset = *offsets.iter().max().unwrap();
    assert!(
        max_offset - min_offset <= 2,
        "Bug #202: Scrollback offset unstable during reads: {:?} (range {}). \
         Expected stable viewport position.",
        offsets, max_offset - min_offset
    );

    send_request(&mut pipe, &Request::CloseSession { session_id: session_id.clone() });
}

/// Bug #202: After scrolling up and receiving output, reading the viewport
/// content at multiple points should always show the same region of scrollback
/// (not intermittently flashing live-view content).
#[test]
#[ntest::timeout(60_000)]
fn test_viewport_content_stable_while_scrolled() {
    let daemon = DaemonFixture::spawn("scroll-content");
    let mut pipe = daemon.connect();

    let session_id = "scroll-content".to_string();
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

    // Generate numbered output to create identifiable scrollback content
    let marker = "CONTENT_INIT_DONE";
    let cmd = format!("for /L %i in (1,1,40) do @echo CONTENT_LINE_%i\r\necho {}\r\n", marker);
    send_request(
        &mut pipe,
        &Request::Write { session_id: session_id.clone(), data: cmd.into_bytes() },
    );
    wait_for_rich_grid_text(&mut pipe, &session_id, marker, Duration::from_secs(10));

    // Scroll up by 15 rows
    let resp = send_request(
        &mut pipe,
        &Request::SetScrollback { session_id: session_id.clone(), offset: 15 },
    );
    assert!(matches!(resp, Response::Ok));

    // Record what the viewport looks like while scrolled up
    let scrolled_grid = read_rich_grid(&mut pipe, &session_id);

    // Now generate new output while scrolled up
    let marker2 = "CONTENT_NEW_DONE";
    let cmd2 = format!("for /L %i in (1,1,15) do @echo FRESH_OUTPUT_%i\r\necho {}\r\n", marker2);
    send_request(
        &mut pipe,
        &Request::Write { session_id: session_id.clone(), data: cmd2.into_bytes() },
    );

    // Wait for new output to be processed
    let start = Instant::now();
    loop {
        let grid = read_rich_grid(&mut pipe, &session_id);
        if grid.total_scrollback > scrolled_grid.total_scrollback + 5 {
            break;
        }
        if start.elapsed() > Duration::from_secs(15) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    // Bug #202: The viewport should NOT contain FRESH_OUTPUT lines.
    // If the daemon preserved the scroll offset, the viewport still shows
    // the old CONTENT_LINE_* entries. If it snapped to bottom, it shows
    // FRESH_OUTPUT_* entries.
    let after_grid = read_rich_grid(&mut pipe, &session_id);
    let after_text = grid_text(&after_grid);

    assert!(
        !after_text.contains("FRESH_OUTPUT_15"),
        "Bug #202: Viewport shows fresh output (FRESH_OUTPUT_15) while user is scrolled 15 rows up. \
         The daemon should show older content from scrollback.\nViewport:\n{}",
        after_text
    );

    assert!(
        after_grid.scrollback_offset >= 15,
        "Bug #202: scrollback_offset={} after new output (expected >= 15). \
         The viewport snapped to the bottom.",
        after_grid.scrollback_offset
    );

    send_request(&mut pipe, &Request::CloseSession { session_id: session_id.clone() });
}
