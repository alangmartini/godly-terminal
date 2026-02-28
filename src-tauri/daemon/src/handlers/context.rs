use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;
use tokio::sync::mpsc;

use godly_protocol::{DaemonMessage, Response};

use crate::session::DaemonSession;

/// Shared context passed to all async request handlers.
pub struct HandlerContext {
    pub sessions: Arc<RwLock<HashMap<String, DaemonSession>>>,
    pub msg_tx: mpsc::Sender<DaemonMessage>,
    pub attached_sessions: Arc<RwLock<Vec<String>>>,
}

/// Helper: look up a session by ID, returning an error response if not found.
/// The closure receives the sessions read-guard and the session reference.
pub fn with_session<F>(ctx: &HandlerContext, session_id: &str, f: F) -> Response
where
    F: FnOnce(&DaemonSession) -> Response,
{
    let sessions_guard = ctx.sessions.read();
    match sessions_guard.get(session_id) {
        Some(session) => f(session),
        None => Response::Error {
            message: format!("Session {} not found", session_id),
        },
    }
}
