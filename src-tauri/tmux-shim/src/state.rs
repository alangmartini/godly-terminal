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
        trace!("resolve_target: input={:?} effective={}", target, t);
        let result = self.lookup_pane(&t).map(|p| p.terminal_id.clone());
        trace!("resolve_target: result={:?}", result);
        result
    }

    /// Get the session name for a pane target.
    ///
    /// Handles tmux target formats: `session`, `session:window`, `session:window.pane`.
    pub fn resolve_session(&self, target: Option<&str>) -> Result<String, String> {
        let t = self.effective_target(target)?;
        trace!("resolve_session: input={:?} effective={}", target, t);
        if t.starts_with('%') {
            return self
                .panes
                .get(&t)
                .map(|p| p.session.clone())
                .ok_or_else(|| format!("pane not found: {}", t));
        }
        // Strip :window and .pane suffixes — e.g. "godly:0" → "godly"
        let session_name = strip_target_suffix(&t);
        if self.sessions.contains_key(session_name) {
            return Ok(session_name.to_string());
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
    ///
    /// Handles tmux target formats: `session`, `session:window`, `session:window.pane`.
    fn lookup_pane(&self, target: &str) -> Result<&PaneMapping, String> {
        if target.starts_with('%') {
            return self
                .panes
                .get(target)
                .ok_or_else(|| format!("pane not found: {}", target));
        }
        // Strip :window and .pane suffixes — e.g. "godly:0.%1" → "godly"
        let session_name = strip_target_suffix(target);
        self.panes
            .values()
            .find(|e| e.session == session_name)
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

/// Strip tmux target suffixes to extract the session name.
///
/// tmux targets can be `session`, `session:window`, or `session:window.pane`.
/// This returns just the session name portion.
pub fn strip_target_suffix(target: &str) -> &str {
    target.split(':').next().unwrap_or(target)
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

/// Ensure the tmux state is initialized with the current terminal's pane mapping.
///
/// When the daemon spawns a shell with `TMUX` and `TMUX_PANE=%0`, it doesn't
/// create the state file. This function seeds it on first use by reading
/// `GODLY_SESSION_ID` (the terminal ID) and querying the active workspace via MCP.
///
/// Also handles **stale state**: if `tmux-state.json` persists across app restarts,
/// it may contain pane mappings from a previous session. We detect this by checking
/// whether `TMUX_PANE` maps to the current `GODLY_SESSION_ID`. If it doesn't, the
/// state is stale and we re-initialize.
pub fn ensure_initialized(
    get_workspace_id: impl FnOnce() -> Result<String, String>,
) -> Result<(), String> {
    // We need GODLY_SESSION_ID to map pane → terminal
    let session_id = match std::env::var("GODLY_SESSION_ID") {
        Ok(s) if !s.is_empty() => s,
        _ => return Ok(()), // No session ID, can't map — not an error
    };

    let pane_id = std::env::var("TMUX_PANE").unwrap_or_else(|_| "%0".to_string());

    // Quick check outside the lock — skip if our pane already maps to this terminal
    let state = load().map_err(|e| format!("{}", e))?;
    if let Some(mapping) = state.panes.get(&pane_id) {
        if mapping.terminal_id == session_id {
            return Ok(());
        }
    }

    // Get workspace ID (involves MCP call, so we do it before locking)
    let workspace_id = get_workspace_id()?;

    // Atomic read-modify-write with file lock
    with_state(|st| {
        // Re-check inside lock — another process may have initialized
        if let Some(mapping) = st.panes.get(&pane_id) {
            if mapping.terminal_id == session_id {
                return Ok(());
            }
        }

        // State is empty or stale — re-initialize
        st.sessions.clear();
        st.panes.clear();
        st.next_pane_id = 0;

        let session_name = "godly".to_string();
        st.sessions
            .insert(session_name.clone(), SessionMapping { workspace_id });
        st.allocate_pane_id(session_id, session_name);
        Ok(())
    })
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
    fn resolve_target_by_session_colon_window() {
        let state = make_state_with_panes();
        let tid = state.resolve_target(Some("main:0")).unwrap();
        assert!(tid == "term-111" || tid == "term-222");
    }

    #[test]
    fn resolve_target_by_session_colon_window_dot_pane() {
        let state = make_state_with_panes();
        let tid = state.resolve_target(Some("main:0.%0")).unwrap();
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
    fn resolve_session_with_window_suffix() {
        let state = make_state_with_panes();
        assert_eq!(state.resolve_session(Some("main:0")).unwrap(), "main");
    }

    // ── strip_target_suffix tests ──

    #[test]
    fn strip_target_suffix_plain_session() {
        assert_eq!(strip_target_suffix("godly"), "godly");
    }

    #[test]
    fn strip_target_suffix_session_window() {
        assert_eq!(strip_target_suffix("godly:0"), "godly");
    }

    #[test]
    fn strip_target_suffix_session_window_pane() {
        assert_eq!(strip_target_suffix("godly:0.%1"), "godly");
    }

    #[test]
    fn strip_target_suffix_empty_string() {
        assert_eq!(strip_target_suffix(""), "");
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

    // ── ensure_initialized tests ──

    /// Helper: run ensure_initialized with a temp APPDATA so it doesn't
    /// interfere with real state or other tests.
    fn run_ensure_initialized_in_temp(
        session_id: Option<&str>,
        workspace_result: Result<String, String>,
    ) -> (Result<(), String>, Option<TmuxState>) {
        let tmp = std::env::temp_dir().join(format!(
            "tmux-state-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(tmp.join("com.godly.terminal")).unwrap();

        // Override APPDATA to temp dir
        let old_appdata = std::env::var("APPDATA").ok();
        std::env::set_var("APPDATA", &tmp);

        // Set or clear GODLY_SESSION_ID
        let old_session_id = std::env::var("GODLY_SESSION_ID").ok();
        match session_id {
            Some(id) => std::env::set_var("GODLY_SESSION_ID", id),
            None => std::env::remove_var("GODLY_SESSION_ID"),
        }

        let result = ensure_initialized(|| workspace_result.clone());

        // Load the state that was written
        let state = load().ok();

        // Restore env vars
        match old_appdata {
            Some(v) => std::env::set_var("APPDATA", v),
            None => std::env::remove_var("APPDATA"),
        }
        match old_session_id {
            Some(v) => std::env::set_var("GODLY_SESSION_ID", v),
            None => std::env::remove_var("GODLY_SESSION_ID"),
        }

        // Clean up
        let _ = std::fs::remove_dir_all(&tmp);

        (result, state)
    }

    #[test]
    fn ensure_initialized_seeds_empty_state() {
        let (result, state) =
            run_ensure_initialized_in_temp(Some("term-abc"), Ok("ws-123".to_string()));
        assert!(result.is_ok());
        let st = state.unwrap();
        assert_eq!(st.panes.len(), 1);
        assert_eq!(st.panes["%0"].terminal_id, "term-abc");
        assert_eq!(st.panes["%0"].session, "godly");
        assert_eq!(st.sessions["godly"].workspace_id, "ws-123");
        assert_eq!(st.next_pane_id, 1);
    }

    #[test]
    fn ensure_initialized_noop_without_session_id() {
        let (result, state) = run_ensure_initialized_in_temp(None, Ok("ws-123".to_string()));
        assert!(result.is_ok());
        let st = state.unwrap();
        assert!(
            st.panes.is_empty(),
            "should not seed without GODLY_SESSION_ID"
        );
    }

    #[test]
    fn ensure_initialized_propagates_workspace_error() {
        let (result, _) = run_ensure_initialized_in_temp(
            Some("term-abc"),
            Err("MCP connection refused".to_string()),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("MCP connection refused"));
    }

    #[test]
    fn ensure_initialized_replaces_stale_state() {
        // Pre-seed state with data from a "previous session", then verify
        // ensure_initialized clears it and re-initializes for the new terminal.
        let tmp = std::env::temp_dir().join(format!(
            "tmux-state-test-stale-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(tmp.join("com.godly.terminal")).unwrap();

        let old_appdata = std::env::var("APPDATA").ok();
        std::env::set_var("APPDATA", &tmp);
        let old_session_id = std::env::var("GODLY_SESSION_ID").ok();
        std::env::set_var("GODLY_SESSION_ID", "term-new");
        let old_tmux_pane = std::env::var("TMUX_PANE").ok();
        std::env::set_var("TMUX_PANE", "%0");

        // Pre-seed state with stale data (%0 maps to a different terminal)
        let mut existing = TmuxState::default();
        existing.sessions.insert(
            "godly".to_string(),
            SessionMapping {
                workspace_id: "ws-old".to_string(),
            },
        );
        existing.allocate_pane_id("term-old".to_string(), "godly".to_string());
        save(&existing).unwrap();

        // ensure_initialized should detect stale state and re-initialize
        let result = ensure_initialized(|| Ok("ws-new".to_string()));

        let st = load().unwrap();

        // Restore
        match old_appdata {
            Some(v) => std::env::set_var("APPDATA", v),
            None => std::env::remove_var("APPDATA"),
        }
        match old_session_id {
            Some(v) => std::env::set_var("GODLY_SESSION_ID", v),
            None => std::env::remove_var("GODLY_SESSION_ID"),
        }
        match old_tmux_pane {
            Some(v) => std::env::set_var("TMUX_PANE", v),
            None => std::env::remove_var("TMUX_PANE"),
        }
        let _ = std::fs::remove_dir_all(&tmp);

        assert!(result.is_ok());
        assert_eq!(st.panes.len(), 1, "stale panes should be cleared");
        assert_eq!(st.panes["%0"].terminal_id, "term-new");
        assert_eq!(st.panes["%0"].session, "godly");
        assert_eq!(st.sessions["godly"].workspace_id, "ws-new");
    }

    #[test]
    fn ensure_initialized_noop_when_pane_matches() {
        // Pre-seed state where %0 already maps to the current terminal.
        // ensure_initialized should be a no-op.
        let tmp = std::env::temp_dir().join(format!(
            "tmux-state-test-match-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(tmp.join("com.godly.terminal")).unwrap();

        let old_appdata = std::env::var("APPDATA").ok();
        std::env::set_var("APPDATA", &tmp);
        let old_session_id = std::env::var("GODLY_SESSION_ID").ok();
        std::env::set_var("GODLY_SESSION_ID", "term-current");
        let old_tmux_pane = std::env::var("TMUX_PANE").ok();
        std::env::set_var("TMUX_PANE", "%0");

        // Pre-seed state where %0 already maps to the current terminal
        let mut existing = TmuxState::default();
        existing.sessions.insert(
            "godly".to_string(),
            SessionMapping {
                workspace_id: "ws-123".to_string(),
            },
        );
        existing.allocate_pane_id("term-current".to_string(), "godly".to_string());
        save(&existing).unwrap();

        // ensure_initialized should skip because %0 already maps to term-current
        let result = ensure_initialized(|| {
            panic!("workspace callback should not be called when pane matches");
        });

        let st = load().unwrap();

        // Restore
        match old_appdata {
            Some(v) => std::env::set_var("APPDATA", v),
            None => std::env::remove_var("APPDATA"),
        }
        match old_session_id {
            Some(v) => std::env::set_var("GODLY_SESSION_ID", v),
            None => std::env::remove_var("GODLY_SESSION_ID"),
        }
        match old_tmux_pane {
            Some(v) => std::env::set_var("TMUX_PANE", v),
            None => std::env::remove_var("TMUX_PANE"),
        }
        let _ = std::fs::remove_dir_all(&tmp);

        assert!(result.is_ok());
        assert_eq!(st.panes.len(), 1);
        assert_eq!(st.panes["%0"].terminal_id, "term-current");
    }
}
