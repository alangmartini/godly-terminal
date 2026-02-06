use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tauri::{AppHandle, Emitter};

use crate::state::ShellType;
use crate::utils::windows_to_wsl_path;

pub struct PtySession {
    master: Arc<parking_lot::Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<parking_lot::Mutex<Box<dyn Write + Send>>>,
    running: Arc<AtomicBool>,
    #[cfg(windows)]
    pid: u32,
    shell_type: ShellType,
}

impl Clone for PtySession {
    fn clone(&self) -> Self {
        Self {
            master: self.master.clone(),
            writer: self.writer.clone(),
            running: self.running.clone(),
            #[cfg(windows)]
            pid: self.pid,
            shell_type: self.shell_type.clone(),
        }
    }
}

impl PtySession {
    pub fn new(
        terminal_id: String,
        working_dir: Option<String>,
        shell_type: ShellType,
        app_handle: AppHandle,
    ) -> Result<Self, String> {
        let pty_system = native_pty_system();

        let size = PtySize {
            rows: 24,
            cols: 80,
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
                if let Some(dir) = &working_dir {
                    cmd.cwd(dir);
                }
                cmd
            }
            ShellType::Wsl { distribution } => {
                let mut cmd = CommandBuilder::new("wsl.exe");
                if let Some(distro) = distribution {
                    cmd.args(["-d", distro]);
                }
                if let Some(dir) = &working_dir {
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

        // Get writer before wrapping master
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to get writer: {}", e))?;

        let master = Arc::new(parking_lot::Mutex::new(pair.master));
        let writer = Arc::new(parking_lot::Mutex::new(writer));
        let running = Arc::new(AtomicBool::new(true));

        // Spawn reader thread
        let reader_master = master.clone();
        let reader_running = running.clone();
        let reader_terminal_id = terminal_id.clone();
        let reader_app_handle = app_handle.clone();

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
                        let _ = reader_app_handle.emit(
                            "terminal-output",
                            serde_json::json!({
                                "terminal_id": reader_terminal_id,
                                "data": data,
                            }),
                        );
                    }
                    Err(_) => break,
                }
            }

            // Terminal closed
            let _ = reader_app_handle.emit(
                "terminal-closed",
                serde_json::json!({
                    "terminal_id": reader_terminal_id,
                }),
            );
        });

        // Keep child handle alive
        thread::spawn(move || {
            let _ = child;
        });

        Ok(Self {
            master,
            writer,
            running,
            #[cfg(windows)]
            pid,
            shell_type,
        })
    }

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

    pub fn close(&self) {
        self.running.store(false, Ordering::Relaxed);
    }

    #[cfg(windows)]
    pub fn get_pid(&self) -> u32 {
        self.pid
    }

    pub fn get_shell_type(&self) -> &ShellType {
        &self.shell_type
    }

    #[cfg(windows)]
    pub fn get_cwd(&self) -> Option<String> {
        use crate::utils::get_process_cwd;
        get_process_cwd(self.pid)
    }
}
