use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use godly_protocol::Response;

use crate::session::DaemonSession;

use super::context::with_session;
use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str, rows: u16, cols: u16) -> Response {
    with_session(ctx, session_id, |session| match session.resize(rows, cols) {
        Ok(()) => Response::Ok,
        Err(e) => Response::Error { message: e },
    })
}

/// Synchronous version for the I/O thread fast path (no HandlerContext available).
pub fn handle_raw(
    sessions: &Arc<RwLock<HashMap<String, DaemonSession>>>,
    session_id: &str,
    rows: u16,
    cols: u16,
) -> Response {
    let sessions_guard = sessions.read();
    match sessions_guard.get(session_id) {
        Some(session) => match session.resize(rows, cols) {
            Ok(()) => Response::Ok,
            Err(e) => Response::Error { message: e },
        },
        None => Response::Error {
            message: format!("Session {} not found", session_id),
        },
    }
}
