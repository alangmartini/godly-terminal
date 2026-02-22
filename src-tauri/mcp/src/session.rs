use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use crate::backend::Backend;
use crate::log::mcp_log;

/// A single MCP session, created on `initialize` and associated with one backend connection.
pub struct McpSession {
    pub backend: Arc<dyn Backend>,
    /// Equivalent to GODLY_SESSION_ID for this MCP session.
    pub terminal_session_id: Option<String>,
    last_activity: RwLock<Instant>,
}

impl McpSession {
    pub fn new(backend: Arc<dyn Backend>, terminal_session_id: Option<String>) -> Self {
        Self {
            backend,
            terminal_session_id,
            last_activity: RwLock::new(Instant::now()),
        }
    }

    /// Update last activity timestamp.
    pub fn touch(&self) {
        if let Ok(mut t) = self.last_activity.write() {
            *t = Instant::now();
        }
    }

    /// Check if session has been idle longer than the given duration.
    pub fn is_idle(&self, max_idle: Duration) -> bool {
        self.last_activity
            .read()
            .map(|t| t.elapsed() > max_idle)
            .unwrap_or(false)
    }
}

/// Registry of active MCP sessions, keyed by session ID string.
pub struct SessionRegistry {
    sessions: RwLock<HashMap<String, Arc<McpSession>>>,
}

impl SessionRegistry {
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new session, returning its ID.
    pub fn create_session(
        &self,
        backend: Arc<dyn Backend>,
        terminal_session_id: Option<String>,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let session = Arc::new(McpSession::new(backend, terminal_session_id));
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.insert(id.clone(), session);
        }
        mcp_log!("session: created session {}", id);
        id
    }

    /// Look up a session by ID.
    pub fn get_session(&self, id: &str) -> Option<Arc<McpSession>> {
        self.sessions
            .read()
            .ok()
            .and_then(|sessions| sessions.get(id).cloned())
    }

    /// Remove a session by ID.
    pub fn remove_session(&self, id: &str) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.remove(id);
            mcp_log!("session: removed session {}", id);
        }
    }

    /// Remove all sessions that have been idle longer than `max_idle`.
    /// Returns the number of sessions removed.
    pub fn cleanup_idle(&self, max_idle: Duration) -> usize {
        let mut removed = 0;
        if let Ok(mut sessions) = self.sessions.write() {
            let before = sessions.len();
            sessions.retain(|id, session| {
                if session.is_idle(max_idle) {
                    mcp_log!("session: cleaning up idle session {}", id);
                    false
                } else {
                    true
                }
            });
            removed = before - sessions.len();
        }
        removed
    }

    /// Get current session count.
    pub fn count(&self) -> usize {
        self.sessions.read().map(|s| s.len()).unwrap_or(0)
    }
}
