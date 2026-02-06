use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use parking_lot::Mutex;
use tauri::{AppHandle, Emitter};

use godly_protocol::{DaemonMessage, Event};

/// Bridge that reads async events from the daemon and emits matching Tauri events.
/// This makes the frontend see identical events as before (terminal-output,
/// terminal-closed, process-changed), so the frontend code is unchanged.
pub struct DaemonBridge {
    running: Arc<AtomicBool>,
}

impl DaemonBridge {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the bridge event reader thread.
    /// The reader is shared with DaemonClient (locked during request/response).
    /// Between requests, the bridge reads events and emits Tauri events.
    pub fn start(
        &self,
        reader: Arc<Mutex<Box<dyn Read + Send>>>,
        app_handle: AppHandle,
    ) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }

        let running = self.running.clone();

        thread::spawn(move || {
            eprintln!("[bridge] Event bridge started");

            while running.load(Ordering::Relaxed) {
                // Try to acquire the reader - the client may hold it during
                // request/response cycles.
                let msg_result = {
                    let mut reader_guard = reader.lock();
                    godly_protocol::read_message::<_, DaemonMessage>(&mut *reader_guard)
                };

                match msg_result {
                    Ok(Some(DaemonMessage::Event(event))) => {
                        emit_event(&app_handle, event);
                    }
                    Ok(Some(DaemonMessage::Response(_))) => {
                        // Responses should be consumed by DaemonClient.send_request()
                        // If we see one here, the client wasn't waiting for it.
                        eprintln!("[bridge] Unexpected response in event loop (dropped)");
                    }
                    Ok(None) => {
                        eprintln!("[bridge] Daemon connection closed");
                        break;
                    }
                    Err(e) => {
                        if running.load(Ordering::Relaxed) {
                            eprintln!("[bridge] Read error: {}", e);
                        }
                        break;
                    }
                }
            }

            eprintln!("[bridge] Event bridge stopped");
        });
    }

    #[allow(dead_code)]
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

/// Emit a daemon event as a Tauri event with the same payload format as before
fn emit_event(app_handle: &AppHandle, event: Event) {
    match event {
        Event::Output { session_id, data } => {
            let _ = app_handle.emit(
                "terminal-output",
                serde_json::json!({
                    "terminal_id": session_id,
                    "data": data,
                }),
            );
        }
        Event::SessionClosed { session_id } => {
            let _ = app_handle.emit(
                "terminal-closed",
                serde_json::json!({
                    "terminal_id": session_id,
                }),
            );
        }
        Event::ProcessChanged {
            session_id,
            process_name,
        } => {
            let _ = app_handle.emit(
                "process-changed",
                serde_json::json!({
                    "terminal_id": session_id,
                    "process_name": process_name,
                }),
            );
        }
    }
}
