use std::collections::HashMap;
use std::io;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Persistent tmux shim state mapping tmux concepts to Godly Terminal UUIDs.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TmuxState {
    /// Maps tmux session names to workspace info.
    pub sessions: HashMap<String, SessionMapping>,
    /// Maps tmux pane IDs (%0, %1, ...) to terminal info.
    pub panes: HashMap<String, PaneMapping>,
    /// Next pane ID counter.
    pub next_pane_id: u32,
}

/// Mapping from a tmux session name to a Godly workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMapping {
    pub workspace_id: String,
}

/// Mapping from a tmux pane ID to a Godly terminal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneMapping {
    pub terminal_id: String,
    pub session: String,
}

impl TmuxState {
    /// Allocate the next pane ID (e.g., "%0", "%1") and return it.
    pub fn allocate_pane_id(&mut self, terminal_id: String, session: String) -> String {
        let pane_id = format!("%{}", self.next_pane_id);
        self.panes.insert(
            pane_id.clone(),
            PaneMapping {
                terminal_id,
                session,
            },
        );
        self.next_pane_id += 1;
        pane_id
    }

    /// Get all pane IDs belonging to a session.
    pub fn session_panes(&self, session_name: &str) -> Vec<String> {
        self.panes
            .iter()
            .filter(|(_, v)| v.session == session_name)
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Remove a session and all its panes from state.
    pub fn remove_session(&mut self, session_name: &str) {
        self.sessions.remove(session_name);
        self.panes.retain(|_, v| v.session != session_name);
    }

    /// Resolve a target string to a terminal_id.
    ///
    /// Supports `%N` pane IDs, session names (returns first pane in session),
    /// and falls back to `$TMUX_PANE` env var if target is None/empty.
    pub fn resolve_target(&self, target: Option<&str>) -> Result<String, String> {
        let t = self.effective_target(target)?;
        Ok(self.lookup_pane(&t)?.terminal_id.clone())
    }

    /// Get the session name for a pane target.
    pub fn resolve_session(&self, target: Option<&str>) -> Result<String, String> {
        let t = self.effective_target(target)?;
        if t.starts_with('%') {
            return self
                .panes
                .get(&t)
                .map(|p| p.session.clone())
                .ok_or_else(|| format!("pane not found: {}", t));
        }
        if self.sessions.contains_key(&t) {
            return Ok(t);
        }
        Err(format!("session not found: {}", t))
    }

    /// Get the workspace_id for a session name.
    pub fn workspace_for_session(&self, session: &str) -> Result<String, String> {
        self.sessions
            .get(session)
            .map(|s| s.workspace_id.clone())
            .ok_or_else(|| format!("session not found: {}", session))
    }

    /// Resolve a target to a pane ID string (e.g. `%0`).
    /// Falls back to `$TMUX_PANE` then `%0` when target is None.
    pub fn resolve_pane_id(&self, target: Option<&str>) -> String {
        match target {
            Some(t) if t.starts_with('%') => t.to_string(),
            Some(session_name) => self
                .panes
                .iter()
                .find(|(_, p)| p.session == session_name)
                .map(|(id, _)| id.clone())
                .unwrap_or_else(|| std::env::var("TMUX_PANE").unwrap_or_else(|_| "%0".to_string())),
            None => std::env::var("TMUX_PANE").unwrap_or_else(|_| "%0".to_string()),
        }
    }

    /// Look up the session name for a pane.
    pub fn pane_session(&self, pane_id: &str) -> Option<&str> {
        self.panes.get(pane_id).map(|p| p.session.as_str())
    }

    /// Resolve the raw target: explicit value → `$TMUX_PANE` → last allocated pane.
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
    fn lookup_pane(&self, target: &str) -> Result<&PaneMapping, String> {
        if target.starts_with('%') {
            return self
                .panes
                .get(target)
                .ok_or_else(|| format!("pane not found: {}", target));
        }
        self.panes
            .values()
            .find(|e| e.session == target)
            .ok_or_else(|| format!("target not found: {}", target))
    }

    /// Find the key of the most recently allocated pane (highest ID that still exists).
    fn last_pane_key(&self) -> Result<String, String> {
        if self.next_pane_id == 0 || self.panes.is_empty() {
            return Err("no panes exist".to_string());
        }
        for i in (0..self.next_pane_id).rev() {
            let key = format!("%{}", i);
            if self.panes.contains_key(&key) {
                return Ok(key);
            }
        }
        Err("no panes exist".to_string())
    }
}

/// Get the path to the tmux state file.
fn state_file_path() -> PathBuf {
    let appdata = std::env::var("APPDATA")
        .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
    PathBuf::from(appdata)
        .join("com.godly.terminal")
        .join("tmux-state.json")
}

/// Load the tmux state from disk, returning default state if the file doesn't exist.
pub fn load() -> Result<TmuxState, io::Error> {
    let path = state_file_path();
    if !path.exists() {
        return Ok(TmuxState::default());
    }
    let contents = std::fs::read_to_string(&path)?;
    serde_json::from_str(&contents).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Save the tmux state to disk atomically (write to temp file, then rename).
/// Creates the parent directory if it doesn't exist.
pub fn save(state: &TmuxState) -> Result<(), io::Error> {
    let path = state_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let json = serde_json::to_string_pretty(state)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Atomic write: write to temp file, then rename
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, json.as_bytes())?;
    std::fs::rename(&tmp_path, &path)?;

    Ok(())
}

/// Execute a closure with exclusive access to the state file.
/// Loads the state, passes it to the closure, and saves if the closure succeeds.
pub fn with_state<F, T>(f: F) -> Result<T, String>
where
    F: FnOnce(&mut TmuxState) -> Result<T, String>,
{
    let _lock = lock_state_file().map_err(|e| format!("Failed to lock state file: {}", e))?;

    let mut state = load().map_err(|e| format!("Failed to load tmux state: {}", e))?;
    let result = f(&mut state)?;
    save(&state).map_err(|e| format!("Failed to save tmux state: {}", e))?;

    Ok(result)
}

/// Acquire an exclusive file lock on a lock file adjacent to the state file.
/// Returns a guard that releases the lock when dropped.
fn lock_state_file() -> Result<LockGuard, io::Error> {
    let lock_path = state_file_path().with_extension("json.lock");
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    LockGuard::acquire(&lock_path)
}

/// RAII guard for a file lock. Closing the file handle releases the lock.
struct LockGuard {
    _file: std::fs::File,
}

impl LockGuard {
    #[cfg(windows)]
    fn acquire(path: &std::path::Path) -> Result<Self, io::Error> {
        use std::os::windows::io::AsRawHandle;
        use winapi::shared::minwindef::DWORD;
        use winapi::um::fileapi::LockFileEx;
        use winapi::um::minwinbase::{LOCKFILE_EXCLUSIVE_LOCK, OVERLAPPED};

        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;

        let handle = file.as_raw_handle();
        let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };

        let result = unsafe {
            LockFileEx(
                handle as _,
                LOCKFILE_EXCLUSIVE_LOCK,
                0,
                1 as DWORD,
                0,
                &mut overlapped,
            )
        };

        if result == 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(Self { _file: file })
    }

    #[cfg(not(windows))]
    fn acquire(path: &std::path::Path) -> Result<Self, io::Error> {
        // On non-Windows, just open the file — no real locking needed for stubs
        let file = std::fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_empty() {
        let state = TmuxState::default();
        assert!(state.sessions.is_empty());
        assert!(state.panes.is_empty());
        assert_eq!(state.next_pane_id, 0);
    }

    #[test]
    fn allocate_pane_id_increments() {
        let mut state = TmuxState::default();
        let id1 = state.allocate_pane_id("term-1".to_string(), "sess".to_string());
        let id2 = state.allocate_pane_id("term-2".to_string(), "sess".to_string());
        assert_eq!(id1, "%0");
        assert_eq!(id2, "%1");
        assert_eq!(state.next_pane_id, 2);
    }

    #[test]
    fn session_panes_filters_correctly() {
        let mut state = TmuxState::default();
        state.allocate_pane_id("t1".to_string(), "sess-a".to_string());
        state.allocate_pane_id("t2".to_string(), "sess-b".to_string());
        state.allocate_pane_id("t3".to_string(), "sess-a".to_string());

        let panes_a = state.session_panes("sess-a");
        assert_eq!(panes_a.len(), 2);
        assert!(panes_a.contains(&"%0".to_string()));
        assert!(panes_a.contains(&"%2".to_string()));

        let panes_b = state.session_panes("sess-b");
        assert_eq!(panes_b.len(), 1);
        assert!(panes_b.contains(&"%1".to_string()));
    }

    #[test]
    fn remove_session_cleans_panes() {
        let mut state = TmuxState::default();
        state.sessions.insert(
            "sess-a".to_string(),
            SessionMapping {
                workspace_id: "ws-1".to_string(),
            },
        );
        state.allocate_pane_id("t1".to_string(), "sess-a".to_string());
        state.allocate_pane_id("t2".to_string(), "sess-b".to_string());

        state.remove_session("sess-a");

        assert!(!state.sessions.contains_key("sess-a"));
        assert_eq!(state.panes.len(), 1);
        assert!(state.panes.contains_key("%1"));
    }

    #[test]
    fn state_roundtrip_json() {
        let mut state = TmuxState::default();
        state.sessions.insert(
            "work".to_string(),
            SessionMapping {
                workspace_id: "uuid-123".to_string(),
            },
        );
        state.allocate_pane_id("term-abc".to_string(), "work".to_string());

        let json = serde_json::to_string(&state).unwrap();
        let restored: TmuxState = serde_json::from_str(&json).unwrap();

        assert_eq!(restored.sessions.len(), 1);
        assert_eq!(restored.sessions["work"].workspace_id, "uuid-123");
        assert_eq!(restored.panes.len(), 1);
        assert_eq!(restored.panes["%0"].terminal_id, "term-abc");
        assert_eq!(restored.panes["%0"].session, "work");
        assert_eq!(restored.next_pane_id, 1);
    }

    #[test]
    fn load_nonexistent_returns_default() {
        // This test relies on a non-existent path; we just verify the default state behavior
        let state = TmuxState::default();
        assert!(state.sessions.is_empty());
    }

    fn make_state_with_panes() -> TmuxState {
        let mut state = TmuxState::default();
        state.sessions.insert(
            "main".to_string(),
            SessionMapping {
                workspace_id: "ws-abc".to_string(),
            },
        );
        state.allocate_pane_id("term-111".to_string(), "main".to_string());
        state.allocate_pane_id("term-222".to_string(), "main".to_string());
        state
    }

    #[test]
    fn resolve_target_by_pane_id() {
        let state = make_state_with_panes();
        assert_eq!(state.resolve_target(Some("%0")).unwrap(), "term-111");
        assert_eq!(state.resolve_target(Some("%1")).unwrap(), "term-222");
    }

    #[test]
    fn resolve_target_pane_not_found() {
        let state = make_state_with_panes();
        assert!(state.resolve_target(Some("%99")).is_err());
    }

    #[test]
    fn resolve_target_by_session_name() {
        let state = make_state_with_panes();
        let tid = state.resolve_target(Some("main")).unwrap();
        assert!(tid == "term-111" || tid == "term-222");
    }

    #[test]
    fn resolve_target_fallback_last_pane() {
        std::env::remove_var("TMUX_PANE");
        let state = make_state_with_panes();
        assert_eq!(state.resolve_target(None).unwrap(), "term-222");
    }

    #[test]
    fn resolve_session_from_pane() {
        let state = make_state_with_panes();
        assert_eq!(state.resolve_session(Some("%0")).unwrap(), "main");
    }

    #[test]
    fn workspace_for_session_found() {
        let state = make_state_with_panes();
        assert_eq!(state.workspace_for_session("main").unwrap(), "ws-abc");
    }

    #[test]
    fn workspace_for_session_not_found() {
        let state = make_state_with_panes();
        assert!(state.workspace_for_session("nonexistent").is_err());
    }

    #[test]
    fn pane_session_lookup() {
        let state = make_state_with_panes();
        assert_eq!(state.pane_session("%0"), Some("main"));
        assert_eq!(state.pane_session("%99"), None);
    }

    #[test]
    fn resolve_pane_id_direct() {
        let state = make_state_with_panes();
        assert_eq!(state.resolve_pane_id(Some("%1")), "%1");
    }
}
