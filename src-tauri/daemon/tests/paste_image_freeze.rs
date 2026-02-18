//! Paste/drag image freeze test: verify the daemon remains responsive after a
//! large write (simulating image data pasted or dragged into the terminal).
//!
//! Bug: When a user pastes or drags data into the terminal while the shell is
//! producing output, a circular deadlock occurs:
//!
//!   1. I/O thread calls session.write(data) → write_all() on ConPTY input
//!   2. ConPTY input pipe fills because shell isn't consuming input (blocked on stdout)
//!   3. Shell stdout blocks because PTY output pipe is full
//!   4. PTY output pipe fills because daemon reader thread stopped reading
//!   5. Reader thread is blocked in blocking_send() — output channel is full
//!   6. Output channel is full because I/O thread can't drain event channel
//!   7. I/O thread can't drain because it's stuck in write_all() (step 1)
//!
//! The result is a complete freeze: no requests processed, no events forwarded,
//! no responses written. The terminal is permanently unresponsive until the
//! deadlock is broken externally (e.g., the shell command finishes).
//!
//! Run with:
//!   cd src-tauri && cargo test -p godly-daemon --test paste_image_freeze -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;
use std::process::{Child, Command};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use godly_protocol::{DaemonMessage, Request, Response, ShellType};

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

/// A duplex pipe client that reads and writes concurrently, mirroring the
/// real application's async DaemonClient/Bridge architecture.
///
/// - A background reader thread continuously reads from the pipe, forwarding
///   Response messages via an std::sync::mpsc channel. Events are counted
///   but discarded (like the real frontend, which re-fetches grid snapshots).
///
/// - The main test thread writes requests via the write handle and receives
///   responses from the channel.
///
/// This prevents the bidirectional named pipe deadlock that would occur if
/// we tried to do both on a single thread (test blocks writing while daemon
/// blocks writing events → both sides stuck).
struct DuplexClient {
    write_pipe: std::fs::File,
    response_rx: mpsc::Receiver<Response>,
    reader_running: Arc<AtomicBool>,
    reader_thread: Option<std::thread::JoinHandle<()>>,
}

impl DuplexClient {
    fn new(pipe: std::fs::File) -> Self {
        let read_pipe = pipe.try_clone().expect("Failed to clone pipe handle");
        let write_pipe = pipe;

        let (response_tx, response_rx) = mpsc::channel::<Response>();
        let reader_running = Arc::new(AtomicBool::new(true));
        let reader_running_clone = reader_running.clone();

        let reader_thread = std::thread::spawn(move || {
            let mut reader = read_pipe;
            while reader_running_clone.load(Ordering::Relaxed) {
                if !pipe_has_data(&reader) {
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }

                match godly_protocol::read_daemon_message(&mut reader) {
                    Ok(Some(DaemonMessage::Response(resp))) => {
                        // Forward responses to the test thread
                        if response_tx.send(resp).is_err() {
                            break; // Test thread dropped receiver
                        }
                    }
                    Ok(Some(DaemonMessage::Event(_))) => {
                        // Discard events (like the real frontend, we just need
                        // to keep reading to prevent pipe backpressure)
                    }
                    Ok(None) => break, // EOF
                    Err(_) => {
                        if reader_running_clone.load(Ordering::Relaxed) {
                            // Read error while still running — pipe may be broken
                            break;
                        }
                    }
                }
            }
        });

        Self {
            write_pipe,
            response_rx,
            reader_running,
            reader_thread: Some(reader_thread),
        }
    }

    /// Send a request and wait for the next Response with a deadline.
    fn send_request(&mut self, request: &Request, deadline: Duration) -> Result<Response, String> {
        godly_protocol::write_request(&mut self.write_pipe, request)
            .map_err(|e| format!("Failed to write request: {}", e))?;

        self.response_rx
            .recv_timeout(deadline)
            .map_err(|e| format!("Response timeout ({:?}): {}", deadline, e))
    }

    /// Send a request without waiting for a response.
    fn send_fire_and_forget(&mut self, request: &Request) {
        godly_protocol::write_request(&mut self.write_pipe, request)
            .expect("Failed to write request");
    }

    /// Wait for a specific response type (Pong) within a deadline.
    /// Skips other response types (e.g., Ok from preceding Write).
    fn wait_for_pong(&mut self, deadline: Duration) -> Result<Duration, String> {
        let start = Instant::now();
        loop {
            let remaining = deadline.checked_sub(start.elapsed()).unwrap_or_default();
            if remaining.is_zero() {
                return Err(format!(
                    "No Pong received within {:?} — I/O thread likely deadlocked",
                    deadline,
                ));
            }

            match self.response_rx.recv_timeout(remaining) {
                Ok(Response::Pong) => return Ok(start.elapsed()),
                Ok(_) => continue, // Skip Ok, Grid, etc.
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(format!(
                        "No Pong received within {:?} — I/O thread likely deadlocked",
                        deadline,
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("Reader thread disconnected".to_string());
                }
            }
        }
    }
}

impl Drop for DuplexClient {
    fn drop(&mut self) {
        self.reader_running.store(false, Ordering::Relaxed);
        if let Some(thread) = self.reader_thread.take() {
            let _ = thread.join();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Maximum time the daemon should take to respond to a Ping after a large write.
/// With the bug, the I/O thread deadlocks and Ping never gets processed.
///
/// 5 seconds is generous — a non-blocking implementation should respond in <100ms.
const RESPONSE_DEADLINE: Duration = Duration::from_secs(5);

/// Bug: Writing a large payload while the shell is producing heavy output
/// causes a circular deadlock in the daemon.
///
/// Deadlock chain:
///   write_all() blocks (ConPTY input full)
///   → I/O thread stuck → can't drain event channel
///   → event channel fills → forwarding task blocks
///   → output channel fills → reader blocks in blocking_send()
///   → reader stops reading PTY → PTY output pipe fills
///   → shell stdout blocks → shell can't consume stdin
///   → ConPTY can't drain input → write_all() stays blocked forever
///
/// Reproduction:
/// 1. Create a session and start a command that produces heavy output
/// 2. While output is flowing, write a large payload (simulating pasted data)
/// 3. Try to Ping the daemon — it should respond within RESPONSE_DEADLINE
///
/// Without fix: deadlock — Ping never arrives.
/// With fix: write should be non-blocking/async so the I/O thread stays responsive.
#[test]
fn test_write_during_heavy_output_deadlocks() {
    let daemon = DaemonFixture::spawn("paste-freeze-output");
    let pipe = daemon.connect();
    let mut client = DuplexClient::new(pipe);

    // Verify daemon is alive
    let resp = client
        .send_request(&Request::Ping, Duration::from_secs(5))
        .expect("Initial ping failed");
    assert!(matches!(resp, Response::Pong));

    let session_id = "paste-output".to_string();
    let resp = client
        .send_request(
            &Request::CreateSession {
                id: session_id.clone(),
                shell_type: ShellType::Cmd,
                cwd: None,
                rows: 24,
                cols: 80,
                env: None,
            },
            Duration::from_secs(10),
        )
        .expect("Create session failed");
    assert!(matches!(resp, Response::SessionCreated { .. }));

    // Attach to session (enables output event forwarding)
    let resp = client
        .send_request(
            &Request::Attach {
                session_id: session_id.clone(),
            },
            Duration::from_secs(10),
        )
        .expect("Attach failed");
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));

    // Wait for cmd.exe prompt
    std::thread::sleep(Duration::from_secs(2));

    // Start a command that produces MASSIVE output.
    // `dir /s C:\Windows\System32` lists thousands of files recursively,
    // generating continuous stdout output for many seconds.
    let resp = client
        .send_request(
            &Request::Write {
                session_id: session_id.clone(),
                data: b"dir /s C:\\Windows\\System32\r\n".to_vec(),
            },
            Duration::from_secs(5),
        )
        .expect("Write dir cmd failed");
    assert!(matches!(resp, Response::Ok));

    // Wait for output to start flowing heavily.
    // The reader thread in DuplexClient continuously drains events from the
    // pipe, preventing the bidirectional pipe deadlock and keeping the daemon's
    // I/O thread free to process requests and forward events. This mirrors the
    // real application architecture.
    std::thread::sleep(Duration::from_secs(3));

    // Bug trigger: write a large payload WHILE output is actively flowing.
    // The I/O thread handles Write directly by calling session.write() → write_all().
    // write_all() writes to ConPTY's input pipe synchronously. While blocked:
    //   - The I/O thread can't drain the event channel
    //   - Events pile up → forwarding task blocks → output channel fills
    //   - Reader thread blocks in blocking_send → stops reading PTY
    //   - PTY output pipe fills → shell blocks on stdout
    //   - Shell can't consume input → ConPTY input pipe backs up
    //   - write_all() is permanently blocked → circular deadlock
    //
    // 1MB payload: enough to overflow ConPTY's input buffer while the shell
    // is busy with dir output and can't consume stdin fast enough.
    client.send_fire_and_forget(&Request::Write {
        session_id: session_id.clone(),
        data: vec![0x41u8; 1024 * 1024], // 1MB of 'A' characters
    });

    // Immediately send a Ping. If the I/O thread is deadlocked, it can't
    // read this request from the pipe.
    client.send_fire_and_forget(&Request::Ping);

    // Wait for Pong. Bug: never arrives because I/O thread is deadlocked.
    let result = client.wait_for_pong(RESPONSE_DEADLINE);
    let latency = result.unwrap_or_else(|e| {
        panic!(
            "PASTE FREEZE DEADLOCK: {}. The I/O thread is deadlocked: \
             write_all() blocks because ConPTY input is full, but ConPTY \
             can't drain because the shell is blocked on stdout, and the \
             reader thread is blocked in blocking_send() because the I/O \
             thread can't drain the event channel.",
            e
        )
    });
    eprintln!(
        "[test] Pong received in {:?} after 1MB write during heavy output",
        latency
    );
}

/// Bug: The deadlock affects ALL sessions on the same connection.
/// Writing a large payload to one session while it's producing output
/// freezes the I/O thread, preventing ANY request for ANY session.
///
/// User-visible: paste image in tab 1, and tabs 2/3/4 all freeze too.
#[test]
fn test_deadlock_affects_all_sessions() {
    let daemon = DaemonFixture::spawn("paste-freeze-cross");
    let pipe = daemon.connect();
    let mut client = DuplexClient::new(pipe);

    let resp = client
        .send_request(&Request::Ping, Duration::from_secs(5))
        .expect("Ping failed");
    assert!(matches!(resp, Response::Pong));

    let paste_session = "paste-target".to_string();
    let other_session = "other-session".to_string();

    for (id, label) in [(&paste_session, "paste"), (&other_session, "other")] {
        let resp = client
            .send_request(
                &Request::CreateSession {
                    id: id.clone(),
                    shell_type: ShellType::Cmd,
                    cwd: None,
                    rows: 24,
                    cols: 80,
                    env: None,
                },
                Duration::from_secs(10),
            )
            .unwrap_or_else(|e| panic!("Create {} session failed: {}", label, e));
        assert!(matches!(resp, Response::SessionCreated { .. }));
    }

    // Attach to paste session (enables output forwarding for deadlock trigger)
    let resp = client
        .send_request(
            &Request::Attach {
                session_id: paste_session.clone(),
            },
            Duration::from_secs(10),
        )
        .expect("Attach failed");
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));
    std::thread::sleep(Duration::from_secs(2));

    // Start heavy output in paste session
    let resp = client
        .send_request(
            &Request::Write {
                session_id: paste_session.clone(),
                data: b"dir /s C:\\Windows\\System32\r\n".to_vec(),
            },
            Duration::from_secs(5),
        )
        .expect("Write dir failed");
    assert!(matches!(resp, Response::Ok));

    // Let output flow
    std::thread::sleep(Duration::from_secs(3));

    // Large write to paste session while output is flowing → deadlock
    client.send_fire_and_forget(&Request::Write {
        session_id: paste_session.clone(),
        data: vec![0x41u8; 1024 * 1024],
    });

    // Try to read grid of the OTHER session. The I/O thread is deadlocked
    // so it can't process ANY request — even for a different session.
    let result = client.send_request(
        &Request::ReadGrid {
            session_id: other_session.clone(),
        },
        RESPONSE_DEADLINE,
    );

    match result {
        Ok(Response::Grid { .. }) => {
            eprintln!("[test] ReadGrid for other session succeeded — no deadlock");
        }
        Ok(other) => {
            eprintln!("[test] ReadGrid returned unexpected: {:?}", other);
        }
        Err(e) => {
            panic!(
                "PASTE FREEZE CROSS-SESSION: {}. A large write to one session \
                 deadlocked the I/O thread, preventing ReadGrid on a DIFFERENT \
                 session. All terminal tabs are frozen.",
                e
            );
        }
    }
}

/// Bug: Binary data (like raw PNG bytes from a failed clipboard paste)
/// combined with heavy output triggers the same deadlock. Binary data
/// contains ESC bytes and control characters that cause the shell to
/// produce particularly verbose error output, amplifying the backpressure.
#[test]
fn test_binary_paste_during_output_deadlocks() {
    let daemon = DaemonFixture::spawn("paste-freeze-binout");
    let pipe = daemon.connect();
    let mut client = DuplexClient::new(pipe);

    let resp = client
        .send_request(&Request::Ping, Duration::from_secs(5))
        .expect("Ping failed");
    assert!(matches!(resp, Response::Pong));

    let session_id = "paste-binout".to_string();
    let resp = client
        .send_request(
            &Request::CreateSession {
                id: session_id.clone(),
                shell_type: ShellType::Cmd,
                cwd: None,
                rows: 24,
                cols: 80,
                env: None,
            },
            Duration::from_secs(10),
        )
        .expect("Create session failed");
    assert!(matches!(resp, Response::SessionCreated { .. }));

    let resp = client
        .send_request(
            &Request::Attach {
                session_id: session_id.clone(),
            },
            Duration::from_secs(10),
        )
        .expect("Attach failed");
    assert!(matches!(resp, Response::Ok | Response::Buffer { .. }));
    std::thread::sleep(Duration::from_secs(2));

    // Start heavy output
    let resp = client
        .send_request(
            &Request::Write {
                session_id: session_id.clone(),
                data: b"dir /s C:\\Windows\\System32\r\n".to_vec(),
            },
            Duration::from_secs(5),
        )
        .expect("Write dir failed");
    assert!(matches!(resp, Response::Ok));

    std::thread::sleep(Duration::from_secs(3));

    // 1MB of binary data (simulating raw PNG image data)
    let mut binary_data: Vec<u8> = Vec::with_capacity(1024 * 1024);
    binary_data.extend_from_slice(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    while binary_data.len() < 1024 * 1024 {
        binary_data.push((binary_data.len() % 256) as u8);
    }

    client.send_fire_and_forget(&Request::Write {
        session_id: session_id.clone(),
        data: binary_data,
    });

    client.send_fire_and_forget(&Request::Ping);

    let result = client.wait_for_pong(RESPONSE_DEADLINE);
    let latency = result.unwrap_or_else(|e| {
        panic!(
            "PASTE FREEZE (binary + output): {}. Raw binary data during \
             heavy shell output deadlocks the daemon's I/O thread via \
             write_all() ↔ blocking_send() circular dependency.",
            e
        )
    });
    eprintln!(
        "[test] Pong received in {:?} after 1MB binary paste during output",
        latency
    );
}
