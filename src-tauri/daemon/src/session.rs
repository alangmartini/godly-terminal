use std::collections::VecDeque;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use godly_protocol::types::ShellType;

const RING_BUFFER_SIZE: usize = 1024 * 1024; // 1MB ring buffer

/// A daemon-managed PTY session that survives app disconnections.
pub struct DaemonSession {
    pub id: String,
    pub shell_type: ShellType,
    pub cwd: Option<String>,
    pub created_at: u64,
    pub rows: u16,
    pub cols: u16,
    #[cfg(windows)]
    pub pid: u32,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    running: Arc<AtomicBool>,
    /// Ring buffer accumulates output when no client is attached
    ring_buffer: Arc<Mutex<VecDeque<u8>>>,
    /// Channel sender for live output to an attached client
    output_tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>>,
}

impl DaemonSession {
    pub fn new(
        id: String,
        shell_type: ShellType,
        cwd: Option<String>,
        rows: u16,
        cols: u16,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        let pair = pty_system
            .openpty(size)
            .map_err(|e| format!("Failed to open pty: {}", e))?;

        let cmd = match &shell_type {
            ShellType::Windows => {
                let mut cmd = CommandBuilder::new("powershell.exe");
                cmd.arg("-NoLogo");
                if let Some(dir) = &cwd {
                    cmd.cwd(dir);
                }
                cmd
            }
            ShellType::Wsl { distribution } => {
                let mut cmd = CommandBuilder::new("wsl.exe");
                if let Some(distro) = distribution {
                    cmd.args(["-d", distro]);
                }
                if let Some(dir) = &cwd {
                    let wsl_path = windows_to_wsl_path(dir);
                    cmd.args(["--cd", &wsl_path]);
                }
                cmd
            }
        };

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("Failed to spawn command: {}", e))?;

        #[cfg(windows)]
        let pid = child.process_id().unwrap_or(0);

        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to get writer: {}", e))?;

        let master = Arc::new(Mutex::new(pair.master));
        let writer = Arc::new(Mutex::new(writer));
        let running = Arc::new(AtomicBool::new(true));
        let ring_buffer = Arc::new(Mutex::new(VecDeque::with_capacity(RING_BUFFER_SIZE)));
        let output_tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>>> =
            Arc::new(Mutex::new(None));

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Spawn reader thread that routes output to either the attached client or ring buffer
        let reader_master = master.clone();
        let reader_running = running.clone();
        let reader_ring = ring_buffer.clone();
        let reader_tx = output_tx.clone();
        let session_id = id.clone();

        thread::spawn(move || {
            let mut reader = {
                let master = reader_master.lock();
                match master.try_clone_reader() {
                    Ok(r) => r,
                    Err(_) => return,
                }
            };

            let mut buf = [0u8; 4096];
            while reader_running.load(Ordering::Relaxed) {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        let data = buf[..n].to_vec();
                        let tx_guard = reader_tx.lock();
                        if let Some(tx) = tx_guard.as_ref() {
                            // Client attached: send live output
                            if tx.send(data).is_err() {
                                // Client disconnected, switch to ring buffer
                                drop(tx_guard);
                                *reader_tx.lock() = None;
                                // Store this chunk in ring buffer
                                let mut ring = reader_ring.lock();
                                append_to_ring(&mut ring, &buf[..n]);
                            }
                        } else {
                            // No client attached: buffer output
                            drop(tx_guard);
                            let mut ring = reader_ring.lock();
                            append_to_ring(&mut ring, &buf[..n]);
                        }
                    }
                    Err(_) => break,
                }
            }

            eprintln!("[daemon] Session {} reader thread exited", session_id);
        });

        // Keep child handle alive
        thread::spawn(move || {
            let _ = child;
        });

        Ok(Self {
            id,
            shell_type,
            cwd,
            created_at: now,
            rows,
            cols,
            #[cfg(windows)]
            pid,
            master,
            writer,
            running,
            ring_buffer,
            output_tx,
        })
    }

    /// Attach a client to this session.
    /// Returns (buffered_data, receiver_for_live_output).
    pub fn attach(&self) -> (Vec<u8>, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

        // Drain ring buffer as initial replay
        let buffered: Vec<u8> = {
            let mut ring = self.ring_buffer.lock();
            ring.drain(..).collect()
        };

        // Set the output sender for live streaming
        *self.output_tx.lock() = Some(tx);

        (buffered, rx)
    }

    /// Detach the current client. Output will accumulate in the ring buffer.
    pub fn detach(&self) {
        *self.output_tx.lock() = None;
    }

    /// Check if a client is currently attached
    pub fn is_attached(&self) -> bool {
        self.output_tx.lock().is_some()
    }

    /// Write data to the PTY
    pub fn write(&self, data: &[u8]) -> Result<(), String> {
        let mut writer = self.writer.lock();
        writer
            .write_all(data)
            .map_err(|e| format!("Failed to write to pty: {}", e))?;
        writer
            .flush()
            .map_err(|e| format!("Failed to flush pty: {}", e))?;
        Ok(())
    }

    /// Resize the PTY
    pub fn resize(&self, rows: u16, cols: u16) -> Result<(), String> {
        let master = self.master.lock();
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to resize pty: {}", e))
    }

    /// Check if the session is still running
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Close the session
    pub fn close(&self) {
        self.running.store(false, Ordering::Relaxed);
        // Drop the output channel to notify attached clients
        *self.output_tx.lock() = None;
    }

    /// Get the session info for protocol messages
    pub fn info(&self) -> godly_protocol::SessionInfo {
        godly_protocol::SessionInfo {
            id: self.id.clone(),
            shell_type: self.shell_type.clone(),
            #[cfg(windows)]
            pid: self.pid,
            #[cfg(not(windows))]
            pid: 0,
            rows: self.rows,
            cols: self.cols,
            cwd: self.cwd.clone(),
            created_at: self.created_at,
            attached: self.is_attached(),
        }
    }

    #[cfg(windows)]
    #[allow(dead_code)]
    pub fn get_pid(&self) -> u32 {
        self.pid
    }

    #[allow(dead_code)]
    pub fn get_shell_type(&self) -> &ShellType {
        &self.shell_type
    }
}

/// Append data to ring buffer, evicting oldest data if necessary
fn append_to_ring(ring: &mut VecDeque<u8>, data: &[u8]) {
    // If data alone exceeds buffer, only keep the tail
    if data.len() >= RING_BUFFER_SIZE {
        ring.clear();
        ring.extend(&data[data.len() - RING_BUFFER_SIZE..]);
        return;
    }

    let needed = ring.len() + data.len();
    if needed > RING_BUFFER_SIZE {
        let to_remove = needed - RING_BUFFER_SIZE;
        ring.drain(..to_remove);
    }
    ring.extend(data);
}

/// Convert a Windows path to WSL path format (duplicated from utils for daemon independence)
fn windows_to_wsl_path(path: &str) -> String {
    let path = path.replace('\\', "/");

    // Check for drive letter pattern: C:/...
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        let drive = path.as_bytes()[0].to_ascii_lowercase() as char;
        let rest = &path[2..];
        return format!("/mnt/{}{}", drive, rest);
    }

    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_to_ring_basic() {
        let mut ring = VecDeque::new();
        append_to_ring(&mut ring, b"hello");
        assert_eq!(ring.iter().copied().collect::<Vec<u8>>(), b"hello");
    }

    #[test]
    fn test_append_to_ring_eviction() {
        let mut ring = VecDeque::new();
        // Fill with data
        let data = vec![0u8; RING_BUFFER_SIZE];
        append_to_ring(&mut ring, &data);
        assert_eq!(ring.len(), RING_BUFFER_SIZE);

        // Append more - should evict oldest
        append_to_ring(&mut ring, b"new");
        assert_eq!(ring.len(), RING_BUFFER_SIZE);
        // Last 3 bytes should be "new"
        let tail: Vec<u8> = ring.iter().rev().take(3).rev().copied().collect();
        assert_eq!(tail, b"new");
    }

    #[test]
    fn test_windows_to_wsl_path() {
        assert_eq!(
            windows_to_wsl_path("C:\\Users\\test"),
            "/mnt/c/Users/test"
        );
        assert_eq!(windows_to_wsl_path("/already/unix"), "/already/unix");
    }
}
