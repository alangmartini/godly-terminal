//! Persistent tmux ID <-> Godly UUID state mapping.
//!
//! State file lives at `%APPDATA%/com.godly.terminal/tmux-state.json`.
//! Atomic writes (write `.tmp`, rename) to avoid corruption.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A pane entry mapping a tmux pane ID to a Godly terminal UUID.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneEntry {
    pub terminal_id: String,
    pub session: String,
}

/// Persistent state mapping tmux concepts to Godly UUIDs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TmuxState {
    /// session_name -> workspace mapping
    pub sessions: HashMap<String, SessionEntry>,
    /// "%N" -> pane entry
    pub panes: HashMap<String, PaneEntry>,
    /// Next pane ID to allocate (monotonically increasing)
    pub next_pane_id: u32,
}

/// A session entry mapping a tmux session name to a Godly workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub workspace_id: String,
}

impl TmuxState {
    /// Path to the state file.
    pub fn state_path() -> PathBuf {
        let base = std::env::var("APPDATA")
            .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
        PathBuf::from(base)
            .join("com.godly.terminal")
            .join("tmux-state.json")
    }

    /// Load state from disk, returning default if file doesn't exist.
    pub fn load() -> Result<Self, String> {
        let path = Self::state_path();
        if !path.exists() {
            return Ok(Self::default());
        }
        let contents =
            std::fs::read_to_string(&path).map_err(|e| format!("Failed to read state: {}", e))?;
        serde_json::from_str(&contents).map_err(|e| format!("Failed to parse state: {}", e))
    }

    /// Save state to disk atomically (write tmp, rename).
    pub fn save(&self) -> Result<(), String> {
        let path = Self::state_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create state dir: {}", e))?;
        }
        let tmp_path = path.with_extension("json.tmp");
        let contents =
            serde_json::to_string_pretty(self).map_err(|e| format!("Failed to serialize: {}", e))?;
        std::fs::write(&tmp_path, &contents)
            .map_err(|e| format!("Failed to write tmp state: {}", e))?;
        std::fs::rename(&tmp_path, &path)
            .map_err(|e| format!("Failed to rename state file: {}", e))?;
        Ok(())
    }

    /// Allocate a new pane ID like `%0`, `%1`, etc.
    pub fn alloc_pane_id(&mut self) -> String {
        let id = format!("%{}", self.next_pane_id);
        self.next_pane_id += 1;
        id
    }

    /// Resolve the raw target string: use the explicit value, fall back to
    /// `$TMUX_PANE`, then fall back to the most recently allocated pane key.
    fn effective_target(&self, target: Option<&str>) -> Result<String, String> {
        match target {
            Some(t) if !t.is_empty() => Ok(t.to_string()),
            _ => {
                let env = std::env::var("TMUX_PANE").unwrap_or_default();
                if !env.is_empty() {
                    return Ok(env);
                }
                self.last_pane_key()
            }
        }
    }

    /// Look up a pane entry by a resolved target string (`%N` or session name).
    fn lookup_pane(&self, target: &str) -> Result<&PaneEntry, String> {
        // %N format — direct pane lookup
        if target.starts_with('%') {
            return self
                .panes
                .get(target)
                .ok_or_else(|| format!("Pane not found: {}", target));
        }

        // Try as session name — find first pane in that session
        self.panes
            .values()
            .find(|e| e.session == target)
            .ok_or_else(|| format!("Target not found: {}", target))
    }

    /// Resolve a target string to a terminal_id.
    ///
    /// Supports:
    /// - `%N` pane IDs (e.g. `%0`, `%1`)
    /// - Session names (returns the first pane in that session)
    /// - Falls back to `$TMUX_PANE` env var if target is empty/None
    pub fn resolve_target(&self, target: Option<&str>) -> Result<String, String> {
        let t = self.effective_target(target)?;
        Ok(self.lookup_pane(&t)?.terminal_id.clone())
    }

    /// Get the session name for a pane target.
    pub fn resolve_session(&self, target: Option<&str>) -> Result<String, String> {
        let t = self.effective_target(target)?;

        // If it's a %N pane, return its session
        if t.starts_with('%') {
            return self
                .panes
                .get(&t)
                .map(|p| p.session.clone())
                .ok_or_else(|| format!("Pane not found: {}", t));
        }

        // Must be a session name — verify it exists
        if self.sessions.contains_key(&t) {
            return Ok(t);
        }

        Err(format!("Session not found: {}", t))
    }

    /// Get the workspace_id for a session name.
    pub fn workspace_for_session(&self, session: &str) -> Result<String, String> {
        self.sessions
            .get(session)
            .map(|s| s.workspace_id.clone())
            .ok_or_else(|| format!("Session not found: {}", session))
    }

    /// Find the key of the most recently allocated pane (highest ID that still exists).
    fn last_pane_key(&self) -> Result<String, String> {
        if self.next_pane_id == 0 || self.panes.is_empty() {
            return Err("No panes exist".to_string());
        }
        for i in (0..self.next_pane_id).rev() {
            let key = format!("%{}", i);
            if self.panes.contains_key(&key) {
                return Ok(key);
            }
        }
        Err("No panes exist".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_state() -> TmuxState {
        let mut state = TmuxState::default();
        state.sessions.insert(
            "main".to_string(),
            SessionEntry {
                workspace_id: "ws-abc".to_string(),
            },
        );
        state.panes.insert(
            "%0".to_string(),
            PaneEntry {
                terminal_id: "term-111".to_string(),
                session: "main".to_string(),
            },
        );
        state.panes.insert(
            "%1".to_string(),
            PaneEntry {
                terminal_id: "term-222".to_string(),
                session: "main".to_string(),
            },
        );
        state.next_pane_id = 2;
        state
    }

    #[test]
    fn resolve_pane_id() {
        let state = make_state();
        assert_eq!(
            state.resolve_target(Some("%0")).unwrap(),
            "term-111"
        );
        assert_eq!(
            state.resolve_target(Some("%1")).unwrap(),
            "term-222"
        );
    }

    #[test]
    fn resolve_pane_id_not_found() {
        let state = make_state();
        assert!(state.resolve_target(Some("%99")).is_err());
    }

    #[test]
    fn resolve_session_name() {
        let state = make_state();
        // Resolving by session name returns the first pane found in that session
        let tid = state.resolve_target(Some("main")).unwrap();
        assert!(tid == "term-111" || tid == "term-222");
    }

    #[test]
    fn resolve_fallback_last_pane() {
        // Clear TMUX_PANE to test fallback
        std::env::remove_var("TMUX_PANE");
        let state = make_state();
        // Should return the last allocated pane (highest ID)
        assert_eq!(state.resolve_target(None).unwrap(), "term-222");
    }

    #[test]
    fn alloc_pane_id_increments() {
        let mut state = TmuxState::default();
        assert_eq!(state.alloc_pane_id(), "%0");
        assert_eq!(state.alloc_pane_id(), "%1");
        assert_eq!(state.alloc_pane_id(), "%2");
        assert_eq!(state.next_pane_id, 3);
    }

    #[test]
    fn workspace_for_session_found() {
        let state = make_state();
        assert_eq!(
            state.workspace_for_session("main").unwrap(),
            "ws-abc"
        );
    }

    #[test]
    fn workspace_for_session_not_found() {
        let state = make_state();
        assert!(state.workspace_for_session("nonexistent").is_err());
    }

    #[test]
    fn resolve_session_from_pane() {
        let state = make_state();
        assert_eq!(state.resolve_session(Some("%0")).unwrap(), "main");
    }

    #[test]
    fn state_roundtrip_serialization() {
        let state = make_state();
        let json = serde_json::to_string(&state).unwrap();
        let loaded: TmuxState = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.next_pane_id, 2);
        assert_eq!(loaded.panes.len(), 2);
        assert_eq!(loaded.sessions.len(), 1);
    }
}
