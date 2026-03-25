//! tmux ID <-> Godly UUID state management.
//!
//! Persists a JSON file mapping tmux-style identifiers (session names, pane
//! IDs like `%0`) to Godly Terminal UUIDs (workspace_id, terminal_id).

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Per-session metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionState {
    pub workspace_id: String,
}

/// Per-pane metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneState {
    pub terminal_id: String,
    pub session: String,
}

/// Root state persisted to disk.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TmuxState {
    pub sessions: HashMap<String, SessionState>,
    pub panes: HashMap<String, PaneState>,
    pub next_pane_id: u32,
}

impl TmuxState {
    /// Path to the state file on disk.
    pub fn path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
        PathBuf::from(base)
            .join("com.godly.terminal")
            .join("tmux-state.json")
    }

    /// Load state from disk. Returns default if file doesn't exist.
    pub fn load() -> Result<Self, String> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read_to_string(&path)
            .map_err(|e| format!("failed to read state file: {}", e))?;
        serde_json::from_str(&data)
            .map_err(|e| format!("failed to parse state file: {}", e))
    }

    /// Write state to disk.
    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create state dir: {}", e))?;
        }
        let data = serde_json::to_string_pretty(self)
            .map_err(|e| format!("failed to serialize state: {}", e))?;
        std::fs::write(&path, data)
            .map_err(|e| format!("failed to write state file: {}", e))
    }

    /// Resolve a target pane spec to a terminal_id.
    ///
    /// Target can be:
    /// - `%N` — direct pane ID
    /// - A session name (returns the first pane in that session)
    /// - Empty/None — uses `$TMUX_PANE` env var
    pub fn resolve_pane(&self, target: Option<&str>) -> Result<String, String> {
        let pane_id = match target {
            Some(t) if t.starts_with('%') => t.to_string(),
            Some(session_name) => {
                // Find first pane belonging to this session
                self.panes
                    .iter()
                    .find(|(_, ps)| ps.session == session_name)
                    .map(|(id, _)| id.clone())
                    .ok_or_else(|| format!("no panes in session '{}'", session_name))?
            }
            None => std::env::var("TMUX_PANE")
                .map_err(|_| "no target specified and $TMUX_PANE not set".to_string())?,
        };

        self.panes
            .get(&pane_id)
            .map(|p| p.terminal_id.clone())
            .ok_or_else(|| format!("pane '{}' not found in state", pane_id))
    }

    /// Resolve a target to a pane ID string (e.g. `%0`).
    ///
    /// Unlike `resolve_pane` which returns the terminal UUID, this returns
    /// the tmux-facing pane ID for use in format string expansion.
    /// Falls back to `$TMUX_PANE` then `%0` when target is None.
    pub fn resolve_pane_id(&self, target: Option<&str>) -> String {
        match target {
            Some(t) if t.starts_with('%') => t.to_string(),
            Some(session_name) => self
                .panes
                .iter()
                .find(|(_, ps)| ps.session == session_name)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| {
                    std::env::var("TMUX_PANE").unwrap_or_else(|_| "%0".to_string())
                }),
            None => std::env::var("TMUX_PANE").unwrap_or_else(|_| "%0".to_string()),
        }
    }

    /// Look up the session name for a pane.
    pub fn pane_session(&self, pane_id: &str) -> Option<&str> {
        self.panes.get(pane_id).map(|p| p.session.as_str())
    }

    /// Get the index of a pane within its session (0-based).
    pub fn pane_index(&self, pane_id: &str) -> usize {
        let session = match self.panes.get(pane_id) {
            Some(p) => &p.session,
            None => return 0,
        };
        let mut session_panes: Vec<&str> = self
            .panes
            .iter()
            .filter(|(_, ps)| ps.session == *session)
            .map(|(id, _)| id.as_str())
            .collect();
        session_panes.sort();
        session_panes
            .iter()
            .position(|&id| id == pane_id)
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> TmuxState {
        let mut state = TmuxState::default();
        state.sessions.insert(
            "main".to_string(),
            SessionState {
                workspace_id: "ws-uuid-1".to_string(),
            },
        );
        state.panes.insert(
            "%0".to_string(),
            PaneState {
                terminal_id: "term-uuid-0".to_string(),
                session: "main".to_string(),
            },
        );
        state.panes.insert(
            "%1".to_string(),
            PaneState {
                terminal_id: "term-uuid-1".to_string(),
                session: "main".to_string(),
            },
        );
        state.next_pane_id = 2;
        state
    }

    #[test]
    fn resolve_direct_pane_id() {
        let state = test_state();
        assert_eq!(state.resolve_pane(Some("%0")).unwrap(), "term-uuid-0");
        assert_eq!(state.resolve_pane(Some("%1")).unwrap(), "term-uuid-1");
    }

    #[test]
    fn resolve_by_session_name() {
        let state = test_state();
        let result = state.resolve_pane(Some("main")).unwrap();
        // Should return one of the panes in session "main"
        assert!(result == "term-uuid-0" || result == "term-uuid-1");
    }

    #[test]
    fn resolve_missing_pane_returns_error() {
        let state = test_state();
        assert!(state.resolve_pane(Some("%99")).is_err());
    }

    #[test]
    fn resolve_missing_session_returns_error() {
        let state = test_state();
        assert!(state.resolve_pane(Some("nonexistent")).is_err());
    }

    #[test]
    fn pane_index_returns_sorted_position() {
        let state = test_state();
        assert_eq!(state.pane_index("%0"), 0);
        assert_eq!(state.pane_index("%1"), 1);
    }

    #[test]
    fn pane_session_lookup() {
        let state = test_state();
        assert_eq!(state.pane_session("%0"), Some("main"));
        assert_eq!(state.pane_session("%99"), None);
    }

    #[test]
    fn default_state_is_empty() {
        let state = TmuxState::default();
        assert!(state.sessions.is_empty());
        assert!(state.panes.is_empty());
        assert_eq!(state.next_pane_id, 0);
    }
}
