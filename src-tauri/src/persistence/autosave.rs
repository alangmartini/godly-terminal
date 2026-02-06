use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tauri::AppHandle;

use crate::persistence::save_layout_internal;
use crate::state::AppState;

/// Auto-save manager that periodically saves state
pub struct AutoSaveManager {
    running: Arc<AtomicBool>,
    dirty: Arc<AtomicBool>,
}

impl AutoSaveManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Mark state as dirty (needing save)
    pub fn mark_dirty(&self) {
        self.dirty.store(true, Ordering::SeqCst);
    }

    /// Start the auto-save background thread
    pub fn start(&self, app_handle: AppHandle, state: Arc<AppState>) {
        if self.running.swap(true, Ordering::SeqCst) {
            return; // Already running
        }

        let running = self.running.clone();
        let dirty = self.dirty.clone();

        thread::spawn(move || {
            eprintln!("[autosave] Background auto-save thread started");

            let periodic_interval = Duration::from_secs(30); // Save every 30 seconds if dirty
            let debounce_delay = Duration::from_secs(2); // Wait 2 seconds after last change
            let mut last_save = Instant::now();

            while running.load(Ordering::SeqCst) {
                thread::sleep(Duration::from_millis(500)); // Check every 500ms

                let is_dirty = dirty.load(Ordering::SeqCst);
                let now = Instant::now();

                // Check if we should save
                let should_save = if is_dirty {
                    // Debounce: wait at least 2 seconds since last change
                    now.duration_since(last_save) >= debounce_delay
                } else {
                    // Periodic save even if not explicitly dirty (catch any missed changes)
                    now.duration_since(last_save) >= periodic_interval
                };

                if should_save {
                    match save_layout_internal(&app_handle, &state) {
                        Ok(()) => {
                            dirty.store(false, Ordering::SeqCst);
                            last_save = Instant::now();
                            if is_dirty {
                                eprintln!("[autosave] State saved (triggered by change)");
                            }
                        }
                        Err(e) => {
                            eprintln!("[autosave] Failed to save: {}", e);
                        }
                    }
                }
            }

            eprintln!("[autosave] Background auto-save thread stopped");
        });
    }

    /// Stop the auto-save thread
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Default for AutoSaveManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AutoSaveManager {
    fn drop(&mut self) {
        self.stop();
    }
}
