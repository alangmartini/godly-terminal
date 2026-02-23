use std::collections::HashMap;

use crate::debug_log::daemon_log;

/// Default scrollback length per session (rows).
pub const DEFAULT_SCROLLBACK_LEN: usize = 10_000;

/// Global scrollback memory budget in bytes (512 MB).
/// When total scrollback across all sessions exceeds this, background
/// (paused) sessions get their scrollback trimmed first.
const GLOBAL_BUDGET_BYTES: usize = 512 * 1024 * 1024;

/// Minimum scrollback length a session can be trimmed to (rows).
/// Even under memory pressure, every session keeps at least this much history.
const MIN_SCROLLBACK_LEN: usize = 500;

/// Per-session scrollback tracking info.
struct SessionInfo {
    scrollback_rows: usize,
    cols: u16,
    is_paused: bool,
    last_output_epoch_ms: u64,
    current_scrollback_len: usize,
}

/// Coordinates scrollback memory across all daemon sessions.
/// Periodically called from the health-check loop to enforce a global budget.
pub struct ScrollbackBudget {
    sessions: HashMap<String, SessionInfo>,
    budget_bytes: usize,
}

impl ScrollbackBudget {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            budget_bytes: GLOBAL_BUDGET_BYTES,
        }
    }

    #[allow(dead_code)]
    pub fn register_session(&mut self, id: String, cols: u16) {
        self.sessions.insert(
            id,
            SessionInfo {
                scrollback_rows: 0,
                cols,
                is_paused: false,
                last_output_epoch_ms: 0,
                current_scrollback_len: DEFAULT_SCROLLBACK_LEN,
            },
        );
    }

    #[allow(dead_code)]
    pub fn remove_session(&mut self, id: &str) {
        self.sessions.remove(id);
    }

    /// Remove entries for sessions that no longer exist.
    pub fn retain_sessions(&mut self, active_ids: &[String]) {
        self.sessions.retain(|id, _| active_ids.contains(id));
    }

    /// Update a session's current stats. Called from the health-check loop.
    /// Auto-registers the session if it's not already tracked.
    pub fn update_session(
        &mut self,
        id: &str,
        scrollback_rows: usize,
        cols: u16,
        is_paused: bool,
        last_output_epoch_ms: u64,
    ) {
        let info = self.sessions.entry(id.to_string()).or_insert_with(|| SessionInfo {
            scrollback_rows: 0,
            cols,
            is_paused: false,
            last_output_epoch_ms: 0,
            current_scrollback_len: DEFAULT_SCROLLBACK_LEN,
        });
        info.scrollback_rows = scrollback_rows;
        info.cols = cols;
        info.is_paused = is_paused;
        info.last_output_epoch_ms = last_output_epoch_ms;
    }

    /// Estimate total scrollback memory across all sessions.
    fn total_memory(&self) -> usize {
        const CELL_BYTES: usize = 32;
        self.sessions
            .values()
            .map(|s| s.scrollback_rows * usize::from(s.cols) * CELL_BYTES)
            .sum()
    }

    /// Check if the global budget is exceeded and compute trimming actions.
    /// Returns a list of (session_id, new_scrollback_len) pairs.
    pub fn check_and_trim(&mut self) -> Vec<(String, usize)> {
        let total = self.total_memory();
        if total <= self.budget_bytes {
            return Vec::new();
        }

        let overshoot = total - self.budget_bytes;
        daemon_log!(
            "ScrollbackBudget: total={:.1}MB, budget={:.1}MB, overshoot={:.1}MB, sessions={}",
            total as f64 / (1024.0 * 1024.0),
            self.budget_bytes as f64 / (1024.0 * 1024.0),
            overshoot as f64 / (1024.0 * 1024.0),
            self.sessions.len()
        );

        // Sort candidates: paused sessions first, then by oldest output time
        let mut candidates: Vec<(&String, &SessionInfo)> = self
            .sessions
            .iter()
            .filter(|(_, s)| s.current_scrollback_len > MIN_SCROLLBACK_LEN)
            .collect();

        candidates.sort_by(|a, b| {
            // Paused first (true > false, so reverse)
            b.1.is_paused
                .cmp(&a.1.is_paused)
                .then_with(|| a.1.last_output_epoch_ms.cmp(&b.1.last_output_epoch_ms))
        });

        let mut freed: usize = 0;
        let mut actions = Vec::new();
        const CELL_BYTES: usize = 32;

        for (id, info) in &candidates {
            if freed >= overshoot {
                break;
            }

            let current_bytes =
                info.scrollback_rows * usize::from(info.cols) * CELL_BYTES;
            if current_bytes == 0 {
                continue;
            }

            // Target: halve scrollback, but not below minimum
            let new_len = (info.current_scrollback_len / 2).max(MIN_SCROLLBACK_LEN);
            if new_len >= info.current_scrollback_len {
                continue;
            }

            let rows_freed = info.scrollback_rows.saturating_sub(new_len);
            let bytes_freed = rows_freed * usize::from(info.cols) * CELL_BYTES;

            actions.push(((*id).clone(), new_len));
            freed += bytes_freed;

            daemon_log!(
                "ScrollbackBudget: trim session {} from {} to {} rows (freed ~{:.1}MB, paused={})",
                id,
                info.current_scrollback_len,
                new_len,
                bytes_freed as f64 / (1024.0 * 1024.0),
                info.is_paused
            );
        }

        // Update our internal tracking for the trimmed sessions
        for (id, new_len) in &actions {
            if let Some(info) = self.sessions.get_mut(id.as_str()) {
                info.current_scrollback_len = *new_len;
            }
        }

        actions
    }
}
