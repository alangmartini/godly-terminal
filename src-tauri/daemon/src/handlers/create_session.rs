use std::collections::HashMap;

use godly_protocol::Response;

use crate::debug_log::daemon_log;
use crate::session::DaemonSession;

use super::HandlerContext;

pub async fn handle(
    ctx: &HandlerContext,
    id: &str,
    shell_type: &godly_protocol::ShellType,
    cwd: &Option<String>,
    rows: u16,
    cols: u16,
    env: &Option<HashMap<String, String>>,
) -> Response {
    match DaemonSession::new(id.to_string(), shell_type.clone(), cwd.clone(), rows, cols, env.clone()) {
        Ok(session) => {
            let info = session.info();
            ctx.sessions.write().insert(id.to_string(), session);
            let session_count = ctx.sessions.read().len();
            eprintln!("[daemon] Created session {}", id);
            daemon_log!("Created session {} (total sessions: {})", id, session_count);
            crate::server::log_memory_usage(&format!("session_create({})", session_count));
            Response::SessionCreated { session: info }
        }
        Err(e) => {
            eprintln!("[daemon] Failed to create session {}: {}", id, e);
            daemon_log!("Failed to create session {}: {}", id, e);
            Response::Error { message: e }
        }
    }
}
