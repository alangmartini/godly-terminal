use godly_protocol::Response;

use crate::debug_log::daemon_log;

use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str) -> Response {
    let mut sessions_guard = ctx.sessions.write();
    match sessions_guard.remove(session_id) {
        Some(session) => {
            session.close();
            ctx.attached_sessions.write().retain(|id| id != session_id);
            let remaining = sessions_guard.len();
            eprintln!("[daemon] Closed session {}", session_id);
            daemon_log!(
                "Closed session {} (remaining sessions: {})",
                session_id,
                remaining
            );
            crate::server::log_memory_usage(&format!("session_close({})", remaining));
            Response::Ok
        }
        None => Response::Error {
            message: format!("Session {} not found", session_id),
        },
    }
}
