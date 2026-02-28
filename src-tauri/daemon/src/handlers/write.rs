use godly_protocol::Response;

use crate::debug_log::daemon_log;

use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str, data: &[u8]) -> Response {
    // Pre-check: reject writes to dead or missing sessions immediately
    // so remote clients get actionable errors instead of silent success.
    {
        let sessions_guard = ctx.sessions.read();
        match sessions_guard.get(session_id) {
            None => {
                return Response::Error {
                    message: format!("Session {} not found", session_id),
                };
            }
            Some(session) if !session.is_running() => {
                return Response::Error {
                    message: format!("Session {} has exited", session_id),
                };
            }
            _ => {}
        }
    }

    // Fire-and-forget: spawn_blocking so write_all() never blocks
    // the async handler or the I/O thread. This breaks the circular
    // deadlock when ConPTY input fills during heavy output.
    // Write ordering is preserved by session.writer Mutex.
    let sessions = ctx.sessions.clone();
    let session_id = session_id.to_string();
    let data = data.to_vec();
    tokio::task::spawn_blocking(move || {
        let sessions_guard = sessions.read();
        if let Some(session) = sessions_guard.get(&session_id) {
            if let Err(e) = session.write(&data) {
                daemon_log!("Write failed for session {}: {}", session_id, e);
            }
        }
    });
    Response::Ok
}
