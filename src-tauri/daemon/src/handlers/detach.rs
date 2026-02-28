use godly_protocol::Response;

use crate::debug_log::daemon_log;

use super::context::with_session;
use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str) -> Response {
    with_session(ctx, session_id, |session| {
        session.detach();
        ctx.attached_sessions.write().retain(|id| id != session_id);
        eprintln!("[daemon] Detached from session {}", session_id);
        daemon_log!("Detached from session {}", session_id);
        Response::Ok
    })
}
