use std::collections::{HashMap, HashSet, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter};

use godly_protocol::{DaemonMessage, Event, Request, Response, read_daemon_message, write_request};

// ── Per-session output stream registry ──────────────────────────────────
//
// Stores raw PTY output bytes per terminal session. The bridge I/O thread
// pushes bytes here on every Event::Output, and the Tauri custom protocol
// handler drains them when the frontend fetches via stream:// URL.
// This bypasses the Tauri event JSON serialization on the output hot path.

/// Maximum buffer size per session (4MB). If the frontend is slow to consume,
/// oldest data is dropped to prevent unbounded memory growth.
const MAX_STREAM_BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Registry of per-session raw output byte buffers.
///
/// Thread-safe: the bridge I/O thread pushes data, and the Tauri custom
/// protocol handler (running on the Tauri thread pool) drains data.
/// Critical sections are short (push a slice, swap a Vec), so contention
/// is negligible.
pub struct OutputStreamRegistry {
    buffers: parking_lot::Mutex<HashMap<String, Vec<u8>>>,
}

impl OutputStreamRegistry {
    pub fn new() -> Self {
        Self {
            buffers: parking_lot::Mutex::new(HashMap::new()),
        }
    }

    /// Append raw PTY output bytes for a session.
    ///
    /// If the buffer exceeds MAX_STREAM_BUFFER_SIZE, oldest data is dropped
    /// to make room. This prevents unbounded memory growth when the frontend
    /// is slow to consume (e.g. heavy output + slow fetch cycle).
    pub fn push(&self, session_id: &str, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let mut buffers = self.buffers.lock();
        let buf = buffers.entry(session_id.to_string()).or_default();
        if buf.len() + data.len() > MAX_STREAM_BUFFER_SIZE {
            // Drop oldest data to make room
            let overflow = (buf.len() + data.len()).saturating_sub(MAX_STREAM_BUFFER_SIZE);
            if overflow >= buf.len() {
                buf.clear();
                // If new data alone exceeds the cap, only keep the tail
                if data.len() > MAX_STREAM_BUFFER_SIZE {
                    buf.extend_from_slice(&data[data.len() - MAX_STREAM_BUFFER_SIZE..]);
                    return;
                }
            } else {
                buf.drain(..overflow);
            }
        }
        buf.extend_from_slice(data);
    }

    /// Drain all accumulated bytes for a session, returning them.
    /// The buffer is cleared after draining.
    pub fn drain(&self, session_id: &str) -> Vec<u8> {
        let mut buffers = self.buffers.lock();
        match buffers.get_mut(session_id) {
            Some(buf) => std::mem::take(buf),
            None => Vec::new(),
        }
    }

    /// Remove a session's buffer entirely (on session close).
    pub fn remove(&self, session_id: &str) {
        self.buffers.lock().remove(session_id);
    }

    /// Number of sessions with active buffers (for diagnostics).
    #[cfg(test)]
    pub fn session_count(&self) -> usize {
        self.buffers.lock().len()
    }
}

// ── Non-blocking event emitter ──────────────────────────────────────────

/// Payload variants for the emitter channel — mirrors protocol::Event but
/// also supports process-changed events from ProcessMonitor.
pub enum EmitPayload {
    /// Notification that new output is available for a terminal.
    /// Does NOT carry the output data — the frontend fetches a grid snapshot
    /// via IPC instead. Omitting data avoids serializing potentially 65KB+
    /// binary payloads as JSON arrays on every chunk.
    TerminalOutput { terminal_id: String },
    /// Pushed grid diff from the daemon — frontend can apply directly without
    /// an IPC round-trip. Suppresses TerminalOutput for the same terminal.
    TerminalGridDiff { terminal_id: String, diff: godly_protocol::types::RichGridDiff },
    TerminalClosed { terminal_id: String, exit_code: Option<i64> },
    ProcessChanged { terminal_id: String, process_name: String },
    TerminalBell { terminal_id: String },
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
    /// Channel capacity of 4096 provides ample headroom for burst output.
    /// The emitter thread coalesces consecutive TerminalOutput events for
    /// the same terminal — under sustained output, only one notification per
    /// terminal is emitted per drain cycle, preventing Tauri event flood.
    pub fn spawn(app_handle: AppHandle) -> Self {
        let (tx, rx) = std::sync::mpsc::sync_channel::<EmitPayload>(4096);
        let dropped = Arc::new(AtomicU64::new(0));

        thread::Builder::new()
            .name("tauri-emitter".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    // Drain all immediately-available events and coalesce
                    // per terminal. GridDiff events supersede TerminalOutput
                    // for the same terminal (the diff carries the data directly).
                    use std::collections::HashMap;
                    let mut output_terminals: HashSet<String> = HashSet::new();
                    let mut diff_terminals: HashMap<String, godly_protocol::types::RichGridDiff> = HashMap::new();
                    let mut other_events: Vec<EmitPayload> = Vec::new();

                    Self::classify_payload(first, &mut output_terminals, &mut diff_terminals, &mut other_events);

                    // Drain any pending events without blocking
                    while let Ok(payload) = rx.try_recv() {
                        Self::classify_payload(payload, &mut output_terminals, &mut diff_terminals, &mut other_events);
                    }

                    // Emit GridDiff for terminals that have one (suppresses TerminalOutput)
                    for (terminal_id, diff) in diff_terminals.drain() {
                        output_terminals.remove(&terminal_id);
                        Self::do_emit(
                            &app_handle,
                            EmitPayload::TerminalGridDiff { terminal_id, diff },
                        );
                    }
                    // Emit TerminalOutput for terminals without a diff (fallback pull path)
                    for terminal_id in output_terminals {
                        Self::do_emit(
                            &app_handle,
                            EmitPayload::TerminalOutput { terminal_id },
                        );
                    }
                    // Emit non-output events in order
                    for payload in other_events {
                        Self::do_emit(&app_handle, payload);
                    }
                }
                eprintln!("[emitter] Tauri-emitter thread exiting (all senders dropped)");
            })
            .expect("Failed to spawn tauri-emitter thread");

        Self { tx, dropped }
    }

    /// Classify a payload: terminal output (coalesced), grid diff (merged),
    /// or non-output event (preserved in order).
    fn classify_payload(
        payload: EmitPayload,
        output_terminals: &mut HashSet<String>,
        diff_terminals: &mut std::collections::HashMap<String, godly_protocol::types::RichGridDiff>,
        other_events: &mut Vec<EmitPayload>,
    ) {
        match payload {
            EmitPayload::TerminalOutput { terminal_id } => {
                output_terminals.insert(terminal_id);
            }
            EmitPayload::TerminalGridDiff { terminal_id, diff } => {
                match diff_terminals.entry(terminal_id) {
                    std::collections::hash_map::Entry::Vacant(e) => { e.insert(diff); }
                    std::collections::hash_map::Entry::Occupied(mut e) => {
                        merge_diffs(e.get_mut(), diff);
                    }
                }
            }
            other => other_events.push(other),
        }
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
            EmitPayload::TerminalOutput { terminal_id } => {
                let _ = app_handle.emit(
                    "terminal-output",
                    serde_json::json!({
                        "terminal_id": terminal_id,
                    }),
                );
            }
            EmitPayload::TerminalGridDiff { terminal_id, diff } => {
                let _ = app_handle.emit(
                    "terminal-grid-diff",
                    serde_json::json!({
                        "terminal_id": terminal_id,
                        "diff": diff,
                    }),
                );
            }
            EmitPayload::TerminalClosed { terminal_id, exit_code } => {
                let _ = app_handle.emit(
                    "terminal-closed",
                    serde_json::json!({
                        "terminal_id": terminal_id,
                        "exit_code": exit_code,
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
            EmitPayload::TerminalBell { terminal_id } => {
                let _ = app_handle.emit(
                    "terminal-bell",
                    serde_json::json!({
                        "terminal_id": terminal_id,
                    }),
                );
            }
        }
    }
}

// ── Bridge debug logger ─────────────────────────────────────────────────

static BRIDGE_LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();
static BRIDGE_START_TIME: OnceLock<Instant> = OnceLock::new();

/// Maximum log file size before rotation (2MB).
const MAX_BRIDGE_LOG_SIZE: u64 = 2 * 1024 * 1024;

fn bridge_log_init() {
    BRIDGE_START_TIME.get_or_init(Instant::now);

    // If the log file is already initialized (e.g. after reconnect), just
    // log a separator — don't reopen. Reopening while the OnceLock holds
    // the old file handle causes a seek-position gap filled with null bytes.
    if BRIDGE_LOG_FILE.get().is_some() {
        bridge_log("=== Bridge re-initialized (reconnect) ===");
        return;
    }

    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir_name = format!(
        "com.godly.terminal{}",
        godly_protocol::instance_suffix()
    );
    let dir = std::path::PathBuf::from(app_data).join(dir_name);
    std::fs::create_dir_all(&dir).ok();

    let path = dir.join("godly-bridge-debug.log");
    let prev_path = dir.join("godly-bridge-debug.prev.log");

    // Rotate if the log file is too large
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_BRIDGE_LOG_SIZE {
            let _ = std::fs::copy(&path, &prev_path);
            let _ = std::fs::remove_file(&path);
        }
    }

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);

    match file {
        Ok(f) => {
            BRIDGE_LOG_FILE.get_or_init(|| Mutex::new(f));
        }
        Err(_) => {
            let fallback = std::env::temp_dir().join("godly-bridge-debug.log");
            if let Ok(f) = OpenOptions::new()
                .create(true)
                .append(true)
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

// ── Wake event for zero-latency I/O thread wakeup ───────────────────────

/// Windows Event object used to wake the bridge I/O thread immediately when
/// a new request arrives. Without this, the I/O thread must wait for its
/// sleep(1ms) to complete before noticing the request, adding up to 1ms of
/// latency per keystroke (or ~15ms without timeBeginPeriod).
#[cfg(windows)]
pub(crate) struct WakeEvent {
    /// Raw HANDLE stored as isize (same pattern as pipe handles elsewhere in the codebase)
    handle: isize,
}

#[cfg(windows)]
unsafe impl Send for WakeEvent {}
#[cfg(windows)]
unsafe impl Sync for WakeEvent {}

#[cfg(windows)]
impl WakeEvent {
    pub fn new() -> Self {
        use winapi::um::synchapi::CreateEventW;
        let handle = unsafe {
            CreateEventW(
                std::ptr::null_mut(), // default security
                0,                    // auto-reset
                0,                    // initially non-signaled
                std::ptr::null(),     // unnamed
            )
        };
        assert!(!handle.is_null(), "CreateEventW failed");
        Self { handle: handle as isize }
    }

    /// Signal the event, waking the waiting thread immediately.
    pub fn signal(&self) {
        use winapi::um::synchapi::SetEvent;
        unsafe {
            SetEvent(self.handle as *mut _);
        }
    }

    /// Wait for the event to be signaled, or until timeout_ms elapses.
    /// Returns true if the event was signaled, false on timeout.
    pub fn wait_timeout(&self, timeout_ms: u32) -> bool {
        use winapi::um::synchapi::WaitForSingleObject;
        use winapi::um::winbase::WAIT_OBJECT_0;
        let result = unsafe { WaitForSingleObject(self.handle as *mut _, timeout_ms) };
        result == WAIT_OBJECT_0
    }
}

#[cfg(windows)]
impl Drop for WakeEvent {
    fn drop(&mut self) {
        use winapi::um::handleapi::CloseHandle;
        unsafe {
            CloseHandle(self.handle as *mut _);
        }
    }
}

#[cfg(not(windows))]
pub(crate) struct WakeEvent;

#[cfg(not(windows))]
impl WakeEvent {
    pub fn new() -> Self {
        Self
    }
    pub fn signal(&self) {}
    pub fn wait_timeout(&self, timeout_ms: u32) -> bool {
        std::thread::sleep(Duration::from_millis(timeout_ms as u64));
        false
    }
}

// ── Bridge implementation ───────────────────────────────────────────────

/// A request to send to the daemon, paired with an optional channel for the response.
/// Fire-and-forget writes set `response_tx = None` to avoid tracking dead channels.
pub struct BridgeRequest {
    pub request: Request,
    pub response_tx: Option<std::sync::mpsc::Sender<Response>>,
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
const MAX_EVENTS_BEFORE_REQUEST_CHECK: usize = 2;

/// Number of idle loop iterations to spin (yield_now) before falling back to sleep.
/// During active I/O this keeps latency near-zero; when truly idle, the sleep
/// prevents burning CPU. On Windows, thread::sleep(1ms) can actually sleep ~15ms
/// due to the default timer resolution (15.625ms), so avoiding sleep during
/// active use is critical for input responsiveness.
const SPIN_BEFORE_SLEEP: u32 = 100;

impl DaemonBridge {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the bridge I/O thread.
    /// Takes ownership of both the pipe reader AND writer, plus channels for
    /// request submission and response routing.
    ///
    /// The `wake_event` is signaled by `send_fire_and_forget` and `try_send_request`
    /// to wake the I/O thread immediately when a new request arrives, instead of
    /// waiting for the sleep timeout.
    ///
    /// If `output_registry` is provided, raw PTY bytes from Event::Output are
    /// pushed into the registry for the Tauri custom protocol stream handler.
    pub fn start(
        &self,
        mut reader: Box<dyn Read + Send>,
        mut writer: Box<dyn Write + Send>,
        request_rx: std::sync::mpsc::Receiver<BridgeRequest>,
        emitter: EventEmitter,
        health: Arc<BridgeHealth>,
        wake_event: Arc<WakeEvent>,
        output_registry: Option<Arc<OutputStreamRegistry>>,
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

            // Adaptive polling: spin (yield_now) for SPIN_BEFORE_SLEEP iterations
            // before falling back to sleep(1ms). This avoids the Windows timer
            // resolution penalty (~15ms) during active I/O while saving CPU when idle.
            let mut idle_count: u32 = 0;

            // Count of fire-and-forget responses to skip (no pending_responses entry).
            // Fire-and-forget writes don't track a response channel, but the daemon
            // still sends Response::Ok. We skip these without routing.
            let mut orphan_responses: usize = 0;

            while running.load(Ordering::Relaxed) {
                let mut did_work = false;
                update_phase(&health, PHASE_IDLE);

                // Step 1: Service ALL pending requests (high priority).
                // Requests (user input, grid snapshots) must be sent before
                // reading events to avoid head-of-line blocking when other
                // sessions produce heavy output.
                loop {
                    update_phase(&health, PHASE_RECV_REQ);
                    match request_rx.try_recv() {
                        Ok(bridge_req) => {
                            update_phase(&health, PHASE_WRITE);
                            let write_start = Instant::now();
                            match write_request(&mut writer, &bridge_req.request) {
                                Ok(()) => {
                                    total_requests_sent += 1;
                                    did_work = true;

                                    match bridge_req.response_tx {
                                        Some(tx) => pending_responses.push_back(tx),
                                        None => orphan_responses += 1,
                                    }

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
                                    running.store(false, Ordering::Relaxed);
                                    break;
                                }
                            }

                            // After writing a request, read the pipe immediately.
                            // The daemon prioritizes responses (high-priority channel),
                            // so the response will be the next message on the pipe.
                            // Read it now before checking more requests.
                            idle_count = 0;
                            loop {
                                update_phase(&health, PHASE_PEEK);
                                match peek_pipe(raw_handle) {
                                    PeekResult::Data => {
                                        update_phase(&health, PHASE_READ);
                                        let read_start = Instant::now();
                                        match read_daemon_message(&mut reader) {
                                            Ok(Some(DaemonMessage::Event(event))) => {
                                                total_events += 1;
                                                update_phase(&health, PHASE_EMIT);
                                                let payload = event_to_payload(event);
                                                if !emitter.try_send(payload) {
                                                    dropped_events += 1;
                                                    blog!("DROPPED EVENT (channel full, total dropped={})", dropped_events);
                                                }
                                                // Keep reading — response hasn't arrived yet
                                                continue;
                                            }
                                            Ok(Some(DaemonMessage::Response(response))) => {
                                                total_responses += 1;
                                                if orphan_responses > 0 {
                                                    orphan_responses -= 1;
                                                } else if let Some(tx) = pending_responses.pop_front() {
                                                    let _ = tx.send(response);
                                                } else {
                                                    eprintln!("[bridge] Got response but no pending request");
                                                    blog!("WARNING: response with no pending request");
                                                }
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
                                    PeekResult::Empty => {
                                        // Response not yet available, yield and retry
                                        thread::yield_now();
                                        continue;
                                    }
                                    PeekResult::Error => {
                                        eprintln!("[bridge] Pipe closed or broken, stopping");
                                        blog!("Pipe peek error, stopping");
                                        running.store(false, Ordering::Relaxed);
                                        break;
                                    }
                                }
                            }

                            if !running.load(Ordering::Relaxed) {
                                break;
                            }
                            // Loop back to check for more queued requests
                            continue;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => break,
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            eprintln!("[bridge] Request channel disconnected, stopping");
                            blog!("Request channel disconnected, stopping");
                            running.store(false, Ordering::Relaxed);
                            break;
                        }
                    }
                }

                if !running.load(Ordering::Relaxed) {
                    break;
                }

                // Step 2: Read up to MAX_EVENTS_BEFORE_REQUEST_CHECK events
                // from the pipe. This prevents pipe buffer buildup from daemon
                // events while keeping the batch small so we quickly loop back
                // to check for new requests.
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
                            update_phase(&health, PHASE_READ);
                            let read_start = Instant::now();
                            match read_daemon_message(&mut reader) {
                                Ok(Some(DaemonMessage::Event(event))) => {
                                    total_events += 1;
                                    events_this_iteration += 1;
                                    did_work = true;

                                    // Push raw output bytes to the stream registry
                                    // (before event_to_payload consumes the event).
                                    // Also clean up on session close.
                                    if let Some(ref registry) = output_registry {
                                        match &event {
                                            Event::Output { session_id, data } => {
                                                registry.push(session_id, data);
                                            }
                                            Event::SessionClosed { session_id, .. } => {
                                                registry.remove(session_id);
                                            }
                                            _ => {}
                                        }
                                    }

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
                                    if orphan_responses > 0 {
                                        orphan_responses -= 1;
                                    } else if let Some(tx) = pending_responses.pop_front() {
                                        let _ = tx.send(response);
                                    } else {
                                        eprintln!(
                                            "[bridge] Got response but no pending request"
                                        );
                                        blog!("WARNING: response with no pending request");
                                    }
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
                            break;
                        }
                    }
                }

                if !did_work {
                    // Adaptive polling: spin briefly, then wait on the wake event.
                    // The wake event is signaled by send_fire_and_forget / try_send_request
                    // when a new request arrives, giving zero-latency wakeup for input.
                    // For incoming pipe data (daemon events), the 1ms timeout ensures we
                    // poll PeekNamedPipe frequently enough.
                    idle_count += 1;
                    if idle_count > SPIN_BEFORE_SLEEP {
                        wake_event.wait_timeout(1);
                    } else {
                        thread::yield_now();
                    }
                } else {
                    idle_count = 0;
                }

                // Periodic stats logging
                if last_stats_time.elapsed() > Duration::from_secs(30) {
                    blog!(
                        "bridge stats: events={}, requests={}, responses={}, dropped_events={}, slow_writes={}, pending={}, orphans={}",
                        total_events,
                        total_requests_sent,
                        total_responses,
                        dropped_events,
                        slow_write_count,
                        pending_responses.len(),
                        orphan_responses
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

/// Merge a newer diff into an existing one for the same terminal.
/// Takes the newer diff's metadata (cursor, dimensions, etc.) and unions dirty rows.
fn merge_diffs(existing: &mut godly_protocol::types::RichGridDiff, newer: godly_protocol::types::RichGridDiff) {
    if newer.full_repaint {
        *existing = newer;
        return;
    }
    if existing.full_repaint {
        // Keep existing rows but update metadata and merge newer rows
        existing.cursor = newer.cursor;
        existing.dimensions = newer.dimensions;
        existing.alternate_screen = newer.alternate_screen;
        existing.cursor_hidden = newer.cursor_hidden;
        existing.scrollback_offset = newer.scrollback_offset;
        existing.total_scrollback = newer.total_scrollback;
        if !newer.title.is_empty() {
            existing.title = newer.title;
        }
        for (idx, row) in newer.dirty_rows {
            if let Some(pos) = existing.dirty_rows.iter().position(|(i, _)| *i == idx) {
                existing.dirty_rows[pos] = (idx, row);
            } else {
                existing.dirty_rows.push((idx, row));
            }
        }
        return;
    }
    // Both partial: merge dirty rows and take newer metadata
    for (idx, row) in newer.dirty_rows {
        if let Some(pos) = existing.dirty_rows.iter().position(|(i, _)| *i == idx) {
            existing.dirty_rows[pos] = (idx, row);
        } else {
            existing.dirty_rows.push((idx, row));
        }
    }
    existing.cursor = newer.cursor;
    existing.dimensions = newer.dimensions;
    existing.alternate_screen = newer.alternate_screen;
    existing.cursor_hidden = newer.cursor_hidden;
    existing.scrollback_offset = newer.scrollback_offset;
    existing.total_scrollback = newer.total_scrollback;
    if !newer.title.is_empty() {
        existing.title = newer.title;
    }
}

/// Convert a protocol Event into an EmitPayload for the non-blocking emitter channel.
fn event_to_payload(event: Event) -> EmitPayload {
    match event {
        Event::Output { session_id, .. } => EmitPayload::TerminalOutput {
            terminal_id: session_id,
        },
        Event::GridDiff { session_id, diff } => EmitPayload::TerminalGridDiff {
            terminal_id: session_id,
            diff,
        },
        Event::SessionClosed { session_id, exit_code } => EmitPayload::TerminalClosed {
            terminal_id: session_id,
            exit_code,
        },
        Event::ProcessChanged {
            session_id,
            process_name,
        } => EmitPayload::ProcessChanged {
            terminal_id: session_id,
            process_name,
        },
        Event::Bell { session_id } => EmitPayload::TerminalBell {
            terminal_id: session_id,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bug: WakeEvent.signal() must immediately unblock WakeEvent.wait_timeout().
    /// Without this, the bridge I/O thread sleeps for the full timeout (1ms with
    /// timeBeginPeriod, ~15ms without) on every keystroke that arrives while idle.
    #[cfg(windows)]
    #[test]
    fn test_wake_event_signal_unblocks_wait() {
        let event = Arc::new(WakeEvent::new());
        let event_clone = Arc::clone(&event);

        // Spawn a thread that waits on the event with a long timeout
        let handle = std::thread::spawn(move || {
            let start = Instant::now();
            // Wait up to 5 seconds — if signal works, this returns immediately
            event_clone.wait_timeout(5000);
            start.elapsed()
        });

        // Give the thread time to enter the wait
        std::thread::sleep(Duration::from_millis(50));

        // Signal the event
        event.signal();

        // The thread should wake up nearly immediately
        let elapsed = handle.join().unwrap();
        assert!(
            elapsed < Duration::from_millis(200),
            "WakeEvent.signal() should unblock wait immediately, but took {:?}",
            elapsed
        );
    }

    /// Verify that wait_timeout returns false (timeout) when not signaled.
    #[cfg(windows)]
    #[test]
    fn test_wake_event_timeout_without_signal() {
        let event = WakeEvent::new();
        let start = Instant::now();
        let signaled = event.wait_timeout(1);
        let elapsed = start.elapsed();

        assert!(!signaled, "Should return false on timeout");
        // Should complete in roughly 1-20ms (timer resolution dependent)
        assert!(
            elapsed < Duration::from_millis(50),
            "Timeout should complete quickly, took {:?}",
            elapsed
        );
    }

    // ── OutputStreamRegistry tests ──────────────────────────────────

    #[test]
    fn registry_push_and_drain() {
        let reg = OutputStreamRegistry::new();
        reg.push("s1", b"hello");
        reg.push("s1", b" world");
        let data = reg.drain("s1");
        assert_eq!(data, b"hello world");
    }

    #[test]
    fn registry_drain_clears_buffer() {
        let reg = OutputStreamRegistry::new();
        reg.push("s1", b"data");
        let _ = reg.drain("s1");
        let data = reg.drain("s1");
        assert!(data.is_empty(), "Buffer should be empty after drain");
    }

    #[test]
    fn registry_drain_unknown_session() {
        let reg = OutputStreamRegistry::new();
        let data = reg.drain("nonexistent");
        assert!(data.is_empty());
    }

    #[test]
    fn registry_push_empty_data_is_noop() {
        let reg = OutputStreamRegistry::new();
        reg.push("s1", b"");
        assert_eq!(reg.session_count(), 0, "Empty push should not create entry");
    }

    #[test]
    fn registry_remove_clears_session() {
        let reg = OutputStreamRegistry::new();
        reg.push("s1", b"data");
        reg.remove("s1");
        let data = reg.drain("s1");
        assert!(data.is_empty());
        assert_eq!(reg.session_count(), 0);
    }

    #[test]
    fn registry_multiple_sessions_isolated() {
        let reg = OutputStreamRegistry::new();
        reg.push("s1", b"one");
        reg.push("s2", b"two");
        assert_eq!(reg.drain("s1"), b"one");
        assert_eq!(reg.drain("s2"), b"two");
    }

    #[test]
    fn registry_overflow_drops_oldest() {
        let reg = OutputStreamRegistry::new();
        // Push data up to the limit
        let chunk = vec![0xAB; MAX_STREAM_BUFFER_SIZE];
        reg.push("s1", &chunk);
        // Push more data — should drop oldest to make room
        reg.push("s1", b"new");
        let data = reg.drain("s1");
        assert_eq!(data.len(), MAX_STREAM_BUFFER_SIZE);
        // The last 3 bytes should be "new"
        assert_eq!(&data[data.len() - 3..], b"new");
    }

    #[test]
    fn registry_overflow_single_oversized_push() {
        let reg = OutputStreamRegistry::new();
        // Push data larger than the max in a single call
        let oversized = vec![0xCD; MAX_STREAM_BUFFER_SIZE + 1000];
        reg.push("s1", &oversized);
        let data = reg.drain("s1");
        assert_eq!(data.len(), MAX_STREAM_BUFFER_SIZE);
        // Should keep the tail of the oversized data
        assert_eq!(data, &oversized[1000..]);
    }

    #[test]
    fn registry_concurrent_push_and_drain() {
        let reg = Arc::new(OutputStreamRegistry::new());
        let reg_push = Arc::clone(&reg);
        let reg_drain = Arc::clone(&reg);

        // Writer thread: push data in a tight loop
        let writer = std::thread::spawn(move || {
            for i in 0..1000u32 {
                reg_push.push("s1", &i.to_le_bytes());
            }
        });

        // Reader thread: drain periodically
        let reader = std::thread::spawn(move || {
            let mut total_bytes = 0;
            for _ in 0..100 {
                total_bytes += reg_drain.drain("s1").len();
                std::thread::yield_now();
            }
            total_bytes
        });

        writer.join().unwrap();
        // Drain any remaining
        let remaining = reg.drain("s1").len();
        let drained = reader.join().unwrap();

        // Total should equal 1000 * 4 bytes per u32
        assert_eq!(
            drained + remaining,
            4000,
            "All pushed bytes should be accounted for"
        );
    }
}
