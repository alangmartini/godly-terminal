//! Bug #393: quick_claude Enter key fails in new worktrees (4th regression).
//!
//! Root cause: `poll_idle` (400ms threshold) fires during a **pause** in Claude
//! Code's startup (e.g., between version banner and MCP initialization), before
//! the ink TUI is actually reading stdin. The prompt text and `\r` are written
//! to the PTY input buffer during this window. When ink finally reads stdin,
//! it receives both text and `\r` in a single chunk, triggering paste detection
//! — which treats `\r` as a literal newline instead of a submit keypress.
//!
//! The 100ms delay between text and `\r` (fix from #185) only works when the
//! TUI is actively reading between the two writes. When the TUI hasn't started
//! reading, both writes accumulate in the PTY buffer regardless of the delay.
//!
//! This test uses a Node.js mock that replicates ink's raw stdin behavior:
//! - Startup delay (not reading stdin) simulating Claude Code initialization
//! - Raw mode stdin that detects whether Enter arrives as a separate chunk
//!   (correct: submit) or merged with text (bug: paste detection)
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test quick_claude_enter -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::process::{Child, Command};
use std::ptr;
use std::thread;
use std::time::{Duration, Instant};

use godly_protocol::frame;
use godly_protocol::types::ShellType;
use godly_protocol::{DaemonMessage, Event, Request, Response};

use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::namedpipeapi::PeekNamedPipe;
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

// ---------------------------------------------------------------------------
// Node.js mock script — simulates Claude Code's ink TUI input handling
// ---------------------------------------------------------------------------

/// Node.js script that mimics ink's raw stdin behavior:
/// 1. Outputs "STARTING" and delays for STARTUP_DELAY_MS (not reading stdin)
/// 2. Switches to raw mode and reads stdin
/// 3. Echoes received text to stdout (like ink renders typed text)
/// 4. If text + Enter arrive in one chunk → "MERGED:<text>" (bug: ink paste)
/// 5. If Enter arrives alone → "SUBMITTED:<text>" (correct: ink submit)
const MOCK_CLAUDE_SCRIPT: &str = r#"
const STARTUP_DELAY_MS = parseInt(process.env.MOCK_STARTUP_DELAY || '4000');

process.stdout.write('MOCK_STARTING\n');

setTimeout(() => {
  process.stdout.write('MOCK_READY\n');

  try {
    process.stdin.setRawMode(true);
  } catch (e) {
    process.stdout.write('RAW_MODE_UNSUPPORTED\n');
    process.exit(3);
  }
  process.stdin.resume();

  let textBuffer = '';

  process.stdin.on('data', (chunk) => {
    const bytes = [...chunk];
    const hasEnter = bytes.includes(0x0D) || bytes.includes(0x0A);
    const printableBytes = bytes.filter(b => b >= 0x20);

    if (hasEnter && printableBytes.length > 0) {
      // Bug #393: Enter arrived in same chunk as text.
      // ink treats this as a paste — \r becomes literal newline, NOT submit.
      const text = Buffer.from(printableBytes).toString();
      process.stdout.write('MERGED:' + text + '\n');
      process.exit(1);
    } else if (hasEnter && textBuffer.length > 0) {
      // Enter arrived alone after text — ink treats as submit keypress.
      process.stdout.write('SUBMITTED:' + textBuffer + '\n');
      process.exit(0);
    } else if (hasEnter) {
      // Enter with no text — unexpected
      process.stdout.write('EMPTY_ENTER\n');
      process.exit(4);
    } else {
      // Text without Enter — accumulate and ECHO to stdout
      // (ink renders typed characters in the input area, which shows up
      // in terminal output — this is what poll_text_in_output detects)
      const text = Buffer.from(printableBytes).toString();
      textBuffer += text;
      process.stdout.write(text);
    }
  });

  // Safety timeout
  setTimeout(() => {
    process.stdout.write('TIMEOUT:' + textBuffer + '\n');
    process.exit(2);
  }, 30000);
}, STARTUP_DELAY_MS);
"#;

// ---------------------------------------------------------------------------
// Helpers (same pattern as ctrl_c_interrupt.rs)
// ---------------------------------------------------------------------------

fn test_pipe_name(test: &str) -> String {
    format!(
        r"\\.\pipe\godly-test-{}-{}",
        test,
        std::process::id()
    )
}

fn daemon_binary_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let deps_dir = exe.parent().unwrap();
    let debug_dir = deps_dir.parent().unwrap();
    let path = debug_dir.join("godly-daemon.exe");
    assert!(
        path.exists(),
        "Daemon binary not found at {:?}. Run `cargo build -p godly-daemon` first.",
        path
    );
    path
}

fn try_connect_pipe(pipe_name: &str) -> Option<std::fs::File> {
    let wide: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            wide.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null_mut(),
            OPEN_EXISTING,
            0,
            ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        None
    } else {
        Some(unsafe { std::fs::File::from_raw_handle(handle as _) })
    }
}

fn pipe_has_data(pipe: &std::fs::File) -> bool {
    let handle = pipe.as_raw_handle();
    let mut bytes_available: u32 = 0;
    let result = unsafe {
        PeekNamedPipe(
            handle as *mut _,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            &mut bytes_available,
            ptr::null_mut(),
        )
    };
    result != 0 && bytes_available > 0
}

fn read_message(pipe: &mut std::fs::File) -> Option<DaemonMessage> {
    frame::read_daemon_message(pipe).ok().flatten()
}

fn send_request_collecting_output(
    pipe: &mut std::fs::File,
    req: &Request,
    session_id: &str,
    output: &mut String,
) -> Response {
    frame::write_request(pipe, req).expect("Failed to write request to pipe");
    loop {
        match read_message(pipe) {
            Some(DaemonMessage::Response(r)) => return r,
            Some(DaemonMessage::Event(Event::Output {
                session_id: sid,
                data,
            })) if sid == session_id => {
                output.push_str(&String::from_utf8_lossy(&data));
            }
            Some(DaemonMessage::Event(_)) => continue,
            None => panic!("Pipe closed while waiting for response"),
        }
    }
}

fn collect_output_until(
    pipe: &mut std::fs::File,
    session_id: &str,
    timeout: Duration,
    predicate: impl Fn(&str) -> bool,
) -> (String, bool) {
    let mut output = String::new();
    let start = Instant::now();

    while start.elapsed() < timeout {
        if pipe_has_data(pipe) {
            match read_message(pipe) {
                Some(DaemonMessage::Event(Event::Output {
                    session_id: sid,
                    data,
                })) if sid == session_id => {
                    output.push_str(&String::from_utf8_lossy(&data));
                    if predicate(&output) {
                        return (output, true);
                    }
                }
                Some(_) => {}
                None => break,
            }
        } else {
            thread::sleep(Duration::from_millis(50));
        }
    }
    let matched = predicate(&output);
    (output, matched)
}

fn wait_for_daemon(pipe_name: &str, timeout: Duration) -> std::fs::File {
    let start = Instant::now();
    loop {
        if let Some(mut file) = try_connect_pipe(pipe_name) {
            if let Ok(()) = frame::write_request(&mut file, &Request::Ping) {
                if let Some(DaemonMessage::Response(Response::Pong)) = read_message(&mut file) {
                    return file;
                }
            }
            drop(file);
        }
        if start.elapsed() > timeout {
            panic!("Daemon did not start within {:?}", timeout);
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn launch_daemon(pipe_name: &str) -> Child {
    use std::os::windows::process::CommandExt;
    let daemon_path = daemon_binary_path();
    Command::new(&daemon_path)
        .env("GODLY_PIPE_NAME", pipe_name)
        .env("GODLY_INSTANCE", pipe_name.trim_start_matches(r"\\.\pipe\"))
        .env("GODLY_NO_DETACH", "1")
        .creation_flags(0x00000200) // CREATE_NEW_PROCESS_GROUP
        .spawn()
        .expect("Failed to spawn daemon")
}

fn kill_daemon(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Simulate poll_idle: poll GetLastOutputTime until idle_ms of silence or timeout.
fn poll_idle(
    pipe: &mut std::fs::File,
    session_id: &str,
    idle_ms: u64,
    timeout_ms: u64,
    output: &mut String,
) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let poll_interval = (idle_ms / 4).min(500).max(50);

    loop {
        // Drain any pending events before sending the request
        while pipe_has_data(pipe) {
            match read_message(pipe) {
                Some(DaemonMessage::Event(Event::Output {
                    session_id: sid,
                    data,
                })) if sid == session_id => {
                    output.push_str(&String::from_utf8_lossy(&data));
                }
                Some(_) => {}
                None => return false,
            }
        }

        let req = Request::GetLastOutputTime {
            session_id: session_id.to_string(),
        };
        frame::write_request(pipe, &req).expect("write GetLastOutputTime");

        loop {
            match read_message(pipe) {
                Some(DaemonMessage::Response(Response::LastOutputTime {
                    epoch_ms, running, ..
                })) => {
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64;
                    let ago = now_ms.saturating_sub(epoch_ms);

                    if ago >= idle_ms || !running {
                        return true;
                    }
                    break;
                }
                Some(DaemonMessage::Event(Event::Output {
                    session_id: sid,
                    data,
                })) if sid == session_id => {
                    output.push_str(&String::from_utf8_lossy(&data));
                }
                Some(_) => {}
                None => return false,
            }
        }

        if Instant::now() >= deadline {
            return false;
        }
        thread::sleep(Duration::from_millis(poll_interval));
    }
}

/// Write the mock Node.js script to a temp file and return its path.
fn write_mock_script() -> std::path::PathBuf {
    let script_path = std::env::temp_dir().join(format!(
        "godly_mock_claude_{}.mjs",
        std::process::id()
    ));
    std::fs::write(&script_path, MOCK_CLAUDE_SCRIPT).expect("Failed to write mock script");
    script_path
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug #393: quick_claude Enter key fails when Claude Code has a startup delay.
///
/// Reproduces the exact failure mechanism:
/// 1. A mock "Claude Code" delays 4s before reading stdin (simulating init)
/// 2. poll_idle detects "idle" during the startup pause (false positive)
/// 3. Prompt text is written to PTY, then \r 100ms later
/// 4. Both writes accumulate in the PTY buffer (TUI not reading yet)
/// 5. When the TUI reads stdin, text + \r arrive as one chunk → paste, not submit
///
/// The test asserts that Enter is recognized as a submit keypress (SUBMITTED),
/// but on the current buggy code, text + Enter merge into one read (MERGED).
#[test]
#[ntest::timeout(120_000)]
fn test_quick_claude_enter_merged_with_text_during_startup_delay() {
    eprintln!("\n=== Bug #393: quick_claude Enter merged with text during startup ===");

    // Check node is available
    let node_check = Command::new("node")
        .arg("--version")
        .output();
    if node_check.is_err() || !node_check.unwrap().status.success() {
        eprintln!("  SKIP: Node.js not available");
        return;
    }

    let script_path = write_mock_script();
    let pipe_name = test_pipe_name("qc-enter");

    let mut daemon = launch_daemon(&pipe_name);
    let mut pipe = wait_for_daemon(&pipe_name, Duration::from_secs(10));

    let session_id = "qc-enter-test";
    let mut output = String::new();

    // Create session with PowerShell
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.to_string(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
        session_id,
        &mut output,
    );
    assert!(
        matches!(resp, Response::SessionCreated { .. }),
        "CreateSession failed: {:?}",
        resp
    );

    // Attach to receive output
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.to_string(),
        },
        session_id,
        &mut output,
    );
    assert!(
        matches!(resp, Response::Ok | Response::Buffer { .. }),
        "Attach failed: {:?}",
        resp
    );

    // Wait for shell prompt
    let (prompt_out, got_prompt) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(15),
        |o| o.contains("PS ") || o.contains("> "),
    );
    output.push_str(&prompt_out);
    eprintln!("  Shell ready: {}", got_prompt);

    // Start the Node.js mock (simulates Claude Code with startup delay)
    let node_cmd = format!(
        "node \"{}\"\r",
        script_path.to_string_lossy().replace('\\', "\\\\")
    );
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: node_cmd.into_bytes(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok), "Write node cmd failed: {:?}", resp);

    // Wait for mock to output "MOCK_STARTING" (confirms script is running)
    let (start_out, mock_started) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(15),
        |o| o.contains("MOCK_STARTING"),
    );
    output.push_str(&start_out);
    eprintln!("  Mock started: {}", mock_started);
    assert!(
        mock_started,
        "Mock Claude script did not start. Output:\n{}",
        output
    );

    // Check if raw mode is unsupported (skip if so)
    thread::sleep(Duration::from_millis(500));
    let (check_out, raw_unsupported) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(1),
        |o| o.contains("RAW_MODE_UNSUPPORTED"),
    );
    output.push_str(&check_out);
    if raw_unsupported {
        eprintln!("  SKIP: Raw mode not supported in this ConPTY environment");
        let _ = send_request_collecting_output(
            &mut pipe,
            &Request::CloseSession { session_id: session_id.to_string() },
            session_id,
            &mut output,
        );
        drop(pipe);
        kill_daemon(&mut daemon);
        std::fs::remove_file(&script_path).ok();
        return;
    }

    // --- Simulate quick_claude_background logic ---
    //
    // The mock has a 4s startup delay. quick_claude uses:
    // - poll_idle with 400ms threshold, 25s timeout
    //
    // poll_idle will detect "idle" during the startup delay because
    // the mock outputs "MOCK_STARTING" and then goes silent for 4s.
    // The 400ms threshold fires during this silence — false positive.

    eprintln!("  Simulating quick_claude: polling for idle (400ms threshold)...");
    let idle_start = Instant::now();
    let idle_detected = poll_idle(
        &mut pipe,
        session_id,
        400,  // Same threshold as quick_claude_background
        25_000,
        &mut output,
    );
    let idle_elapsed = idle_start.elapsed();
    eprintln!(
        "  poll_idle returned {} after {:?}",
        idle_detected, idle_elapsed
    );

    // Small delay like quick_claude (Step 4)
    thread::sleep(Duration::from_millis(300));

    // Step 5: Write prompt text (separate from Enter)
    let prompt_text = "test prompt from bug 393";
    eprintln!("  Writing prompt text: {:?}", prompt_text);
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: prompt_text.as_bytes().to_vec(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok), "Write text failed: {:?}", resp);

    // Step 5b: Wait for prompt text to appear in terminal output (echo).
    // Bug #393 fix: instead of a fixed 100ms delay, poll SearchBuffer until
    // the TUI has echoed the text back — confirming it's actively reading stdin.
    // Only then is it safe to send Enter as a separate write.
    let search_prefix: String = prompt_text.chars().take(40).collect();
    eprintln!("  Polling SearchBuffer for echo of: {:?}", search_prefix);
    let echo_deadline = Instant::now() + Duration::from_secs(30);
    let mut echo_found = false;
    while Instant::now() < echo_deadline {
        let req = Request::SearchBuffer {
            session_id: session_id.to_string(),
            text: search_prefix.clone(),
            strip_ansi: true,
        };
        let resp = send_request_collecting_output(
            &mut pipe,
            &req,
            session_id,
            &mut output,
        );
        if matches!(resp, Response::SearchResult { found: true, .. }) {
            echo_found = true;
            eprintln!("  Echo detected in output buffer!");
            break;
        }
        thread::sleep(Duration::from_millis(250));
    }
    if !echo_found {
        eprintln!("  WARNING: Echo not detected within 30s, sending Enter anyway");
    }

    // Step 5c: Small buffer after echo detection, then send Enter
    thread::sleep(Duration::from_millis(200));
    eprintln!("  Sending Enter (\\r) as separate write...");
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: b"\r".to_vec(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok), "Write Enter failed: {:?}", resp);

    // Wait for the mock's verdict: SUBMITTED (correct) or MERGED (bug)
    let (result_out, got_result) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(30),
        |o| {
            o.contains("SUBMITTED:")
                || o.contains("MERGED:")
                || o.contains("EMPTY_ENTER")
                || o.contains("TIMEOUT:")
                || o.contains("RAW_MODE_UNSUPPORTED")
        },
    );
    output.push_str(&result_out);

    // Cleanup
    let _ = send_request_collecting_output(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
        session_id,
        &mut output,
    );
    drop(pipe);
    kill_daemon(&mut daemon);
    std::fs::remove_file(&script_path).ok();

    // --- Assertions ---
    eprintln!("  Result output: {:?}", &result_out[..result_out.len().min(500)]);
    eprintln!("  Full session output ({} bytes)", output.len());

    assert!(
        got_result,
        "Mock script did not produce a result within timeout.\n\
         This likely means the writes never reached the mock.\n\
         Full output:\n{}",
        output
    );

    // The test MUST see "SUBMITTED:" to confirm Enter was recognized as a
    // submit keypress (separate read from text). If we see "MERGED:", it
    // means text + Enter arrived in one chunk — ink would treat \r as a
    // literal newline in a paste, not as a submit. This is the bug.
    let submitted = output.contains("SUBMITTED:");
    let merged = output.contains("MERGED:");

    assert!(
        submitted && !merged,
        "\n\n\
         Bug #393: quick_claude Enter key merged with prompt text.\n\
         \n\
         poll_idle detected 'idle' after {:?} (during Claude Code startup delay),\n\
         then wrote prompt text + Enter (\\r) with a 100ms gap. But because\n\
         the TUI wasn't reading stdin yet (still initializing), both writes\n\
         accumulated in the PTY input buffer. When the TUI started reading,\n\
         it received text + \\r as a single chunk — triggering ink's paste\n\
         detection, which treats \\r as a literal newline instead of submit.\n\
         \n\
         Expected: SUBMITTED:{}\n\
         Got:      {}\n\
         \n\
         The 100ms delay between text and \\r (fix from #185) only works when\n\
         the TUI is actively reading stdin between the two writes. When the\n\
         TUI hasn't started reading, writes accumulate regardless of delay.\n\
         \n\
         Full output:\n{}\n",
        idle_elapsed,
        prompt_text,
        if merged { "MERGED (ink paste detection)" }
        else if output.contains("TIMEOUT:") { "TIMEOUT (writes never reached mock)" }
        else if output.contains("EMPTY_ENTER") { "EMPTY_ENTER (Enter arrived without text)" }
        else { "UNKNOWN" },
        output
    );
}

/// Bug #393 (regression guard): poll_idle gives false positive during startup.
///
/// Documents the known limitation of poll_idle: its 400ms threshold fires
/// during a normal gap in a program's startup output, even though the program
/// hasn't finished initializing. The fix (#393) works around this by using
/// SearchBuffer echo detection instead of relying solely on poll_idle.
///
/// This test verifies that the workaround is necessary: poll_idle DOES fire
/// prematurely during a startup gap, confirming that echo-based detection
/// (poll_text_in_output) is the correct approach.
#[test]
#[ntest::timeout(60_000)]
fn test_poll_idle_false_positive_during_startup_gap() {
    eprintln!("\n=== Bug #393: poll_idle false positive during startup output gap ===");

    let pipe_name = test_pipe_name("qc-idle");
    let mut daemon = launch_daemon(&pipe_name);
    let mut pipe = wait_for_daemon(&pipe_name, Duration::from_secs(10));

    let session_id = "qc-idle-test";
    let mut output = String::new();

    // Create and attach session
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::CreateSession {
            id: session_id.to_string(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows: 24,
            cols: 80,
            env: None,
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Attach {
            session_id: session_id.to_string(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Wait for shell prompt
    let (prompt_out, _) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(15),
        |o| o.contains("PS ") || o.contains("> "),
    );
    output.push_str(&prompt_out);

    // Run a command that outputs, pauses, then outputs more
    // (simulates Claude Code: version banner → MCP init pause → TUI ready)
    let cmd = "Write-Host 'PHASE1_START'; Start-Sleep -Seconds 2; Write-Host 'PHASE2_READY'\r";
    let resp = send_request_collecting_output(
        &mut pipe,
        &Request::Write {
            session_id: session_id.to_string(),
            data: cmd.as_bytes().to_vec(),
        },
        session_id,
        &mut output,
    );
    assert!(matches!(resp, Response::Ok));

    // Wait for PHASE1 output
    let (phase1_out, got_phase1) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(10),
        |o| o.contains("PHASE1_START"),
    );
    output.push_str(&phase1_out);
    assert!(got_phase1, "PHASE1 never appeared. Output:\n{}", output);
    eprintln!("  PHASE1 output received");

    // Now poll_idle with 400ms threshold — this will fire during the 2s gap
    // because there's no output for >400ms, even though the program is still running.
    let idle_start = Instant::now();
    let idle_detected = poll_idle(
        &mut pipe,
        session_id,
        400,
        10_000,
        &mut output,
    );
    let idle_elapsed = idle_start.elapsed();
    eprintln!("  poll_idle returned {} after {:?}", idle_detected, idle_elapsed);

    // Wait for PHASE2 to eventually appear (proves the program wasn't done)
    let (phase2_out, got_phase2) = collect_output_until(
        &mut pipe,
        session_id,
        Duration::from_secs(10),
        |o| o.contains("PHASE2_READY"),
    );
    output.push_str(&phase2_out);
    eprintln!("  PHASE2 appeared: {}", got_phase2);

    // Cleanup
    let _ = send_request_collecting_output(
        &mut pipe,
        &Request::CloseSession {
            session_id: session_id.to_string(),
        },
        session_id,
        &mut output,
    );
    drop(pipe);
    kill_daemon(&mut daemon);

    // Regression guard: poll_idle MUST fire prematurely during startup gaps.
    // This confirms that echo-based detection (poll_text_in_output) is needed
    // as a workaround. If poll_idle somehow stops firing prematurely (e.g.,
    // the threshold is increased), this test will need updating.
    assert!(
        idle_detected && idle_elapsed < Duration::from_secs(2),
        "\n\n\
         Regression guard: poll_idle did NOT fire prematurely.\n\
         \n\
         Expected: poll_idle returns true within <2s (during the startup gap).\n\
         Got: idle_detected={}, elapsed={:?}\n\
         \n\
         If poll_idle no longer fires prematurely, the echo-based workaround\n\
         in quick_claude_background (poll_text_in_output) may no longer be\n\
         the only safeguard — but it should be kept as defense-in-depth.\n\
         \n\
         PHASE2 appeared: {}\n",
        idle_detected,
        idle_elapsed,
        got_phase2,
    );
}
