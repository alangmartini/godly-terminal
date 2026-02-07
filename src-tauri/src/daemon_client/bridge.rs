use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use tauri::{AppHandle, Emitter};

use godly_protocol::{DaemonMessage, Event, Request, Response};

/// A request to send to the daemon, paired with a channel for the response.
pub struct BridgeRequest {
    pub request: Request,
    pub response_tx: mpsc::Sender<Response>,
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
        request_rx: mpsc::Receiver<BridgeRequest>,
        app_handle: AppHandle,
    ) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }

        let running = self.running.clone();

        thread::spawn(move || {
            eprintln!("[bridge] Event bridge started");

            // Get the raw handle from the reader for PeekNamedPipe
            let raw_handle = get_raw_handle(&reader);

            // FIFO queue of response channels — one per in-flight request.
            // Responses from the daemon arrive in the same order as requests.
            let mut pending_responses: VecDeque<mpsc::Sender<Response>> = VecDeque::new();

            while running.load(Ordering::Relaxed) {
                // Step 1: Check if there are bytes available to read (non-blocking)
                match peek_pipe(raw_handle) {
                    PeekResult::Data => {
                        // Read the message
                        match godly_protocol::read_message::<_, DaemonMessage>(&mut reader) {
                            Ok(Some(DaemonMessage::Event(event))) => {
                                emit_event(&app_handle, event);
                            }
                            Ok(Some(DaemonMessage::Response(response))) => {
                                // Route response to the oldest waiting caller (FIFO)
                                if let Some(tx) = pending_responses.pop_front() {
                                    let _ = tx.send(response);
                                } else {
                                    eprintln!("[bridge] Got response but no pending request");
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
                        continue; // Check for more data immediately
                    }
                    PeekResult::Error => {
                        eprintln!("[bridge] Pipe closed or broken, stopping");
                        break;
                    }
                    PeekResult::Empty => {
                        // Fall through to check for outgoing requests
                    }
                }

                // Step 2: Check if there are requests to send
                match request_rx.try_recv() {
                    Ok(bridge_req) => {
                        // Write the request to the pipe
                        match godly_protocol::write_message(&mut writer, &bridge_req.request) {
                            Ok(()) => {
                                pending_responses.push_back(bridge_req.response_tx);
                            }
                            Err(e) => {
                                eprintln!("[bridge] Write error: {}, stopping", e);
                                // Don't send error response — breaking drops all
                                // pending_responses, which signals callers via RecvError
                                break;
                            }
                        }
                        continue; // Check for response immediately
                    }
                    Err(mpsc::TryRecvError::Empty) => {
                        // Nothing to do - brief sleep to avoid busy-waiting
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        eprintln!("[bridge] Request channel disconnected, stopping");
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
