use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use godly_protocol::Response;

use crate::session::DaemonSession;

use super::context::with_session;
use super::HandlerContext;

/// Async handler (fallback if request reaches the async handler loop).
pub async fn handle(ctx: &HandlerContext, session_id: &str) -> Response {
    with_session(ctx, session_id, |session| {
        session.resume();
        Response::Ok
    })
}

/// Synchronous version for the I/O thread fast path (no HandlerContext available).
pub fn handle_raw(
    sessions: &Arc<RwLock<HashMap<String, DaemonSession>>>,
    session_id: &str,
) -> Response {
    let sessions_guard = sessions.read();
    match sessions_guard.get(session_id) {
        Some(session) => {
            session.resume();
            Response::Ok
        }
        None => Response::Error {
            message: format!("Session {} not found", session_id),
        },
    }
}
