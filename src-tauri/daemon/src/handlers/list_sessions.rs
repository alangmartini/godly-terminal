use godly_protocol::Response;

use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext) -> Response {
    let sessions_guard = ctx.sessions.read();
    let list: Vec<_> = sessions_guard.values().map(|s| s.info()).collect();
    Response::SessionList { sessions: list }
}
