#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;
#[cfg(windows)]
use winapi::shared::minwindef::{DWORD, FALSE, MAX_PATH};
#[cfg(windows)]
use winapi::um::handleapi::CloseHandle;
#[cfg(windows)]
use winapi::um::processthreadsapi::OpenProcess;
#[cfg(windows)]
use winapi::um::psapi::GetModuleBaseNameW;
#[cfg(windows)]
use winapi::um::tlhelp32::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
#[cfg(windows)]
use winapi::um::winnt::PROCESS_QUERY_INFORMATION;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::state::{AppState, ShellType};

pub struct ProcessMonitor {
    running: Arc<AtomicBool>,
}

impl ProcessMonitor {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn start(&self, app_handle: AppHandle, state: Arc<AppState>) {
        if self.running.swap(true, Ordering::Relaxed) {
            return;
        }

        let running = self.running.clone();

        thread::spawn(move || {
            let mut last_processes: HashMap<String, String> = HashMap::new();

            while running.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_secs(1));

                let sessions = state.pty_sessions.read();
                for (terminal_id, session) in sessions.iter() {
                    #[cfg(windows)]
                    {
                        // For WSL terminals, we can't query Linux process names from Windows
                        // Instead, display the distribution name or "wsl"
                        let process_name = match session.get_shell_type() {
                            ShellType::Wsl { distribution } => {
                                distribution.clone().unwrap_or_else(|| String::from("wsl"))
                            }
                            ShellType::Windows => {
                                // For Windows terminals, query the actual foreground process
                                let pid = session.get_pid();
                                match get_foreground_process(pid) {
                                    Some(name) => name,
                                    None => continue,
                                }
                            }
                        };

                        let last = last_processes.get(terminal_id);
                        if last.map(|s| s.as_str()) != Some(&process_name) {
                            last_processes.insert(terminal_id.clone(), process_name.clone());
                            state.update_terminal_process(terminal_id, process_name.clone());
                            let _ = app_handle.emit(
                                "process-changed",
                                serde_json::json!({
                                    "terminal_id": terminal_id,
                                    "process_name": process_name,
                                }),
                            );
                        }
                    }

                    #[cfg(not(windows))]
                    {
                        let _ = session;
                    }
                }
            }
        });
    }

    #[allow(dead_code)]
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

#[cfg(windows)]
fn get_foreground_process(parent_pid: u32) -> Option<String> {
    // Find the deepest child process
    let child_pid = find_deepest_child(parent_pid)?;
    get_process_name(child_pid)
}

#[cfg(windows)]
fn find_deepest_child(parent_pid: u32) -> Option<u32> {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() {
            return Some(parent_pid);
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as DWORD;

        let mut children: Vec<u32> = Vec::new();

        if Process32FirstW(snapshot, &mut entry) != FALSE {
            loop {
                if entry.th32ParentProcessID == parent_pid {
                    children.push(entry.th32ProcessID);
                }
                if Process32NextW(snapshot, &mut entry) == FALSE {
                    break;
                }
            }
        }

        CloseHandle(snapshot);

        if children.is_empty() {
            Some(parent_pid)
        } else {
            // Recursively find the deepest child
            children
                .into_iter()
                .filter_map(|pid| find_deepest_child(pid))
                .next()
        }
    }
}

#[cfg(windows)]
fn get_process_name(pid: u32) -> Option<String> {
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION, FALSE, pid);
        if handle.is_null() {
            return None;
        }

        let mut name_buf = [0u16; MAX_PATH];
        let len = GetModuleBaseNameW(
            handle,
            std::ptr::null_mut(),
            name_buf.as_mut_ptr(),
            MAX_PATH as DWORD,
        );

        CloseHandle(handle);

        if len == 0 {
            return None;
        }

        let name = OsString::from_wide(&name_buf[..len as usize]);
        let name_str = name.to_string_lossy().to_string();

        // Remove .exe extension
        Some(name_str.trim_end_matches(".exe").to_string())
    }
}

impl Default for ProcessMonitor {
    fn default() -> Self {
        Self::new()
    }
}
