use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Emitter};

use godly_protocol::{DaemonMessage, Event, Response};

/// Bridge that reads all messages from the daemon pipe and routes them:
/// - Response messages → sent to DaemonClient via mpsc channel
/// - Event messages → emitted as Tauri events to the frontend
///
/// The bridge is the **sole reader** of the pipe, eliminating lock contention.
pub struct DaemonBridge {
    running: Arc<AtomicBool>,
}

impl DaemonBridge {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the bridge reader thread.
    /// Takes ownership of the pipe reader and the response sender channel.
    pub fn start(
        &self,
        mut reader: Box<dyn Read + Send>,
        response_tx: mpsc::Sender<Response>,
        app_handle: AppHandle,
    ) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }

        let running = self.running.clone();

        thread::spawn(move || {
            eprintln!("[bridge] Event bridge started");

            while running.load(Ordering::Relaxed) {
                let msg_result =
                    godly_protocol::read_message::<_, DaemonMessage>(&mut reader);

                match msg_result {
                    Ok(Some(DaemonMessage::Event(event))) => {
                        emit_event(&app_handle, event);
                    }
                    Ok(Some(DaemonMessage::Response(response))) => {
                        // Route response back to the DaemonClient
                        if response_tx.send(response).is_err() {
                            eprintln!("[bridge] Response channel closed, stopping");
                            break;
                        }
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
