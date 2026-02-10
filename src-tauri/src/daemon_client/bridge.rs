use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};

use godly_protocol::{DaemonMessage, Event, Request, Response};

// ── Non-blocking event emitter ──────────────────────────────────────────

/// Payload variants for the emitter channel — mirrors protocol::Event but
/// also supports process-changed events from ProcessMonitor.
pub enum EmitPayload {
    TerminalOutput { terminal_id: String, data: Vec<u8> },
    TerminalClosed { terminal_id: String },
    ProcessChanged { terminal_id: String, process_name: String },
}

/// Cloneable handle that enqueues Tauri emit calls into a bounded channel.
/// A dedicated thread drains the channel and performs the actual `app_handle.emit()`.
/// This makes `try_send()` non-blocking (~sub-microsecond), so the bridge I/O
/// thread is immune to main-thread stalls.
#[derive(Clone)]
pub struct EventEmitter {
    tx: std::sync::mpsc::SyncSender<EmitPayload>,
    dropped: Arc<AtomicU64>,
}

impl EventEmitter {
    /// Spawn the emitter thread and return a cloneable handle.
    /// Channel capacity of 256 ≈ ~4s of headroom at 60 events/s.
    pub fn spawn(app_handle: AppHandle) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<EmitPayload>(256);
        let dropped = Arc::new(AtomicU64::new(0));

        thread::Builder::new()
            .name("tauri-emitter".into())
            .spawn(move || {
                while let Ok(payload) = rx.recv() {
                    Self::do_emit(&app_handle, payload);
                }
                eprintln!("[emitter] Tauri-emitter thread exiting (all senders dropped)");
            })
            .expect("Failed to spawn tauri-emitter thread");

        Self { tx, dropped }
    }

    /// Non-blocking enqueue. Returns true if sent, false if the channel is full
    /// (in which case the event is dropped and the counter incremented).
    pub fn try_send(&self, payload: EmitPayload) -> bool {
        match self.tx.try_send(payload) {
            Ok(()) => true,
            Err(std::sync::mpsc::TrySendError::Full(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
            Err(std::sync::mpsc::TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
                false
            }
        }
    }

    /// Number of events dropped because the channel was full.
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    /// Perform the actual Tauri emit for one payload.
    fn do_emit(app_handle: &AppHandle, payload: EmitPayload) {
        match payload {
            EmitPayload::TerminalOutput { terminal_id, data } => {
                let _ = app_handle.emit(
                    "terminal-output",
                    serde_json::json!({
                        "terminal_id": terminal_id,
                        "data": data,
                    }),
                );
            }
            EmitPayload::TerminalClosed { terminal_id } => {
                let _ = app_handle.emit(
                    "terminal-closed",
                    serde_json::json!({
                        "terminal_id": terminal_id,
                    }),
                );
            }
            EmitPayload::ProcessChanged { terminal_id, process_name } => {
                let _ = app_handle.emit(
                    "process-changed",
                    serde_json::json!({
                        "terminal_id": terminal_id,
                        "process_name": process_name,
                    }),
                );
            }
        }
    }
}

// ── Bridge debug logger ─────────────────────────────────────────────────

static BRIDGE_LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static BRIDGE_START_TIME: OnceLock<Instant> = OnceLock::new();

fn bridge_log_init() {
    BRIDGE_START_TIME.get_or_init(Instant::now);

    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir_name = format!(
        "com.godly.terminal{}",
        godly_protocol::instance_suffix()
    );
    let dir = std::path::PathBuf::from(app_data).join(dir_name);
    std::fs::create_dir_all(&dir).ok();

    let path = dir.join("godly-bridge-debug.log");
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path);

    match file {
        Ok(f) => {
            BRIDGE_LOG_FILE.get_or_init(|| Mutex::new(f));
        }
        Err(_) => {
            let fallback = std::env::temp_dir().join("godly-bridge-debug.log");
            if let Ok(f) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&fallback)
            {
                BRIDGE_LOG_FILE.get_or_init(|| Mutex::new(f));
            }
        }
    }
}

pub(crate) fn bridge_log(msg: &str) {
    if let Some(mutex) = BRIDGE_LOG_FILE.get() {
        if let Ok(mut file) = mutex.lock() {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let elapsed = BRIDGE_START_TIME
                .get()
                .map(|s| s.elapsed())
                .unwrap_or_default();
            let _ = writeln!(
                file,
                "[{}.{:03}] [{:>8.3}s] {}",
                ts.as_secs(),
                ts.subsec_millis(),
                elapsed.as_secs_f64(),
                msg
            );
            let _ = file.flush();
        }
    }
}

macro_rules! blog {
    ($($arg:tt)*) => {
        bridge_log(&format!($($arg)*))
    };
}

// ── Bridge health / phase tracking ──────────────────────────────────────

pub const PHASE_IDLE: u8 = 0;
pub const PHASE_PEEK: u8 = 1;
pub const PHASE_READ: u8 = 2;
pub const PHASE_EMIT: u8 = 3;
pub const PHASE_RECV_REQ: u8 = 4;
pub const PHASE_WRITE: u8 = 5;
pub const PHASE_STOPPED: u8 = 6;

pub fn phase_name(phase: u8) -> &'static str {
    match phase {
        PHASE_IDLE => "idle",
        PHASE_PEEK => "peek_pipe",
        PHASE_READ => "read_message",
        PHASE_EMIT => "emit_event",
        PHASE_RECV_REQ => "recv_request",
        PHASE_WRITE => "write_message",
        PHASE_STOPPED => "stopped",
        _ => "unknown",
    }
}

pub struct BridgeHealth {
    pub current_phase: AtomicU8,
    pub last_activity_ms: AtomicU64,
}

impl BridgeHealth {
    pub fn new() -> Self {
        Self {
            current_phase: AtomicU8::new(PHASE_IDLE),
            last_activity_ms: AtomicU64::new(0),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn update_phase(health: &BridgeHealth, phase: u8) {
    health.current_phase.store(phase, Ordering::Relaxed);
    health.last_activity_ms.store(now_ms(), Ordering::Relaxed);
}

// ── Bridge implementation ───────────────────────────────────────────────

/// A request to send to the daemon, paired with a channel for the response.
pub struct BridgeRequest {
    pub request: Request,
    pub response_tx: std::sync::mpsc::Sender<Response>,
}

/// Bridge that owns the pipe's reader AND writer, performing all I/O in a single thread.
///
/// On Windows, synchronous named pipe I/O is serialized per file object.
/// Since DuplicateHandle creates handles to the SAME file object, concurrent
/// reads and writes from different threads deadlock. The bridge solves this by
/// doing all pipe I/O from one thread using PeekNamedPipe for non-blocking reads.
pub struct DaemonBridge {
    running: Arc<AtomicBool>,
}

/// Maximum number of events to read before checking for pending outgoing requests.
/// This prevents high-throughput output from starving user input writes.
const MAX_EVENTS_BEFORE_REQUEST_CHECK: usize = 8;

impl DaemonBridge {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the bridge I/O thread.
    /// Takes ownership of both the pipe reader AND writer, plus channels for
    /// request submission and response routing.
    pub fn start(
        &self,
        mut reader: Box<dyn Read + Send>,
        mut writer: Box<dyn Write + Send>,
        request_rx: std::sync::mpsc::Receiver<BridgeRequest>,
        emitter: EventEmitter,
        health: Arc<BridgeHealth>,
    ) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }

        bridge_log_init();
        blog!("=== Bridge starting ===");

        let running = self.running.clone();

        thread::spawn(move || {
            eprintln!("[bridge] Event bridge started");
            blog!("Event bridge I/O thread started");

            // Get the raw handle from the reader for PeekNamedPipe
            let raw_handle = get_raw_handle(&reader);
            blog!("Pipe handle={}", raw_handle);

            // FIFO queue of response channels — one per in-flight request.
            // Responses from the daemon arrive in the same order as requests.
            let mut pending_responses: VecDeque<std::sync::mpsc::Sender<Response>> =
                VecDeque::new();

            // Stats for periodic logging
            let mut total_events: u64 = 0;
            let mut total_requests_sent: u64 = 0;
            let mut total_responses: u64 = 0;
            let mut dropped_events: u64 = 0;
            let mut slow_write_count: u64 = 0;
            let mut last_stats_time = Instant::now();

            while running.load(Ordering::Relaxed) {
                let mut did_work = false;
                update_phase(&health, PHASE_IDLE);

                // Step 1: Read incoming messages from daemon, but limit batch size.
                // After reading MAX_EVENTS_BEFORE_REQUEST_CHECK events, check for
                // pending requests to avoid starving user input during heavy output.
                let mut events_this_iteration = 0;
                loop {
                    if events_this_iteration >= MAX_EVENTS_BEFORE_REQUEST_CHECK {
                        blog!(
                            "Event batch limit hit ({}), checking for pending requests",
                            events_this_iteration
                        );
                        break;
                    }

                    update_phase(&health, PHASE_PEEK);
                    match peek_pipe(raw_handle) {
                        PeekResult::Data => {
                            // Read the message
                            update_phase(&health, PHASE_READ);
                            let read_start = Instant::now();
                            match godly_protocol::read_message::<_, DaemonMessage>(&mut reader) {
                                Ok(Some(DaemonMessage::Event(event))) => {
                                    total_events += 1;
                                    events_this_iteration += 1;
                                    did_work = true;

                                    update_phase(&health, PHASE_EMIT);
                                    let payload = event_to_payload(event);
                                    if !emitter.try_send(payload) {
                                        dropped_events += 1;
                                        blog!("DROPPED EVENT (channel full, total dropped={})", dropped_events);
                                    }
                                }
                                Ok(Some(DaemonMessage::Response(response))) => {
                                    total_responses += 1;
                                    did_work = true;
                                    // Route response to the oldest waiting caller (FIFO)
                                    if let Some(tx) = pending_responses.pop_front() {
                                        let _ = tx.send(response);
                                    } else {
                                        eprintln!(
                                            "[bridge] Got response but no pending request"
                                        );
                                        blog!("WARNING: response with no pending request");
                                    }
                                    // Always break after a response to let the request
                                    // sender proceed, which may trigger another request
                                    break;
                                }
                                Ok(None) => {
                                    eprintln!("[bridge] Daemon connection closed");
                                    blog!("Daemon connection closed (EOF)");
                                    running.store(false, Ordering::Relaxed);
                                    break;
                                }
                                Err(e) => {
                                    if running.load(Ordering::Relaxed) {
                                        let elapsed = read_start.elapsed();
                                        eprintln!("[bridge] Read error: {}", e);
                                        blog!(
                                            "Read error after {:.1}ms: {}",
                                            elapsed.as_secs_f64() * 1000.0,
                                            e
                                        );
                                    }
                                    running.store(false, Ordering::Relaxed);
                                    break;
                                }
                            }
                        }
                        PeekResult::Error => {
                            eprintln!("[bridge] Pipe closed or broken, stopping");
                            blog!("Pipe peek error, stopping");
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                        PeekResult::Empty => {
                            break; // No more data to read
                        }
                    }
                }

                if !running.load(Ordering::Relaxed) {
                    break;
                }

                // Step 2: Check if there are requests to send
                update_phase(&health, PHASE_RECV_REQ);
                match request_rx.try_recv() {
                    Ok(bridge_req) => {
                        update_phase(&health, PHASE_WRITE);
                        let write_start = Instant::now();
                        // Write the request to the pipe
                        match godly_protocol::write_message(&mut writer, &bridge_req.request) {
                            Ok(()) => {
                                total_requests_sent += 1;
                                did_work = true;
                                pending_responses.push_back(bridge_req.response_tx);

                                let elapsed = write_start.elapsed();
                                if elapsed > Duration::from_millis(50) {
                                    slow_write_count += 1;
                                    blog!(
                                        "SLOW REQUEST WRITE: {:?} took {:.1}ms (slow #{})",
                                        std::mem::discriminant(&bridge_req.request),
                                        elapsed.as_secs_f64() * 1000.0,
                                        slow_write_count
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("[bridge] Write error: {}, stopping", e);
                                blog!("Write error: {}", e);
                                // Don't send error response — breaking drops all
                                // pending_responses, which signals callers via RecvError
                                break;
                            }
                        }
                        // After writing a request, loop back to read the response
                        continue;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // Nothing to send
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        eprintln!("[bridge] Request channel disconnected, stopping");
                        blog!("Request channel disconnected, stopping");
                        break;
                    }
                }

                if !did_work {
                    // Nothing to do - brief sleep to avoid busy-waiting
                    thread::sleep(Duration::from_millis(1));
                }

                // Periodic stats logging
                if last_stats_time.elapsed() > Duration::from_secs(30) {
                    blog!(
                        "bridge stats: events={}, requests={}, responses={}, dropped_events={}, slow_writes={}, pending={}",
                        total_events,
                        total_requests_sent,
                        total_responses,
                        dropped_events,
                        slow_write_count,
                        pending_responses.len()
                    );
                    last_stats_time = Instant::now();
                }
            }

            update_phase(&health, PHASE_STOPPED);

            // Drain all pending response channels so callers get an error
            // instead of blocking forever on recv().
            let pending_count = pending_responses.len();
            for tx in pending_responses.drain(..) {
                let _ = tx.send(Response::Error {
                    message: "Bridge disconnected".to_string(),
                });
            }
            if pending_count > 0 {
                blog!("Drained {} pending response channels with error", pending_count);
            }

            blog!(
                "bridge stopped: events={}, requests={}, responses={}, dropped_events={}, slow_writes={}",
                total_events,
                total_requests_sent,
                total_responses,
                dropped_events,
                slow_write_count
            );
            eprintln!("[bridge] Event bridge stopped");
        });
    }

    #[allow(dead_code)]
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Get the raw handle from a boxed reader (assumes it wraps a std::fs::File)
#[cfg(windows)]
fn get_raw_handle(reader: &Box<dyn Read + Send>) -> isize {
    use std::os::windows::io::AsRawHandle;
    // The reader is a Box<dyn Read + Send> that wraps a File.
    // We need the raw handle for PeekNamedPipe.
    //
    // Safety: We know the reader is a File because we created it that way.
    let reader_ptr = &**reader as *const dyn Read as *const std::fs::File;
    unsafe { (*reader_ptr).as_raw_handle() as isize }
}

#[cfg(not(windows))]
fn get_raw_handle(_reader: &Box<dyn Read + Send>) -> isize {
    0
}

/// Result of a non-blocking pipe peek: data available, empty, or error (pipe closed/broken).
enum PeekResult {
    Data,
    Empty,
    Error,
}

/// Non-blocking check: how many bytes are available to read from the pipe?
#[cfg(windows)]
fn peek_pipe(handle: isize) -> PeekResult {
    use winapi::um::namedpipeapi::PeekNamedPipe;

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
    if result == 0 {
        return PeekResult::Error; // Pipe closed or broken
    }
    if bytes_available > 0 {
        PeekResult::Data
    } else {
        PeekResult::Empty
    }
}

#[cfg(not(windows))]
fn peek_pipe(_handle: isize) -> PeekResult {
    PeekResult::Empty
}

/// Convert a protocol Event into an EmitPayload for the non-blocking emitter channel.
fn event_to_payload(event: Event) -> EmitPayload {
    match event {
        Event::Output { session_id, data } => EmitPayload::TerminalOutput {
            terminal_id: session_id,
            data,
        },
        Event::SessionClosed { session_id } => EmitPayload::TerminalClosed {
            terminal_id: session_id,
        },
        Event::ProcessChanged {
            session_id,
            process_name,
        } => EmitPayload::ProcessChanged {
            terminal_id: session_id,
            process_name,
        },
    }
}
