use godly_protocol::Response;

use super::context::with_session;
use super::HandlerContext;

pub async fn handle(
    ctx: &HandlerContext,
    session_id: &str,
    text: &str,
    strip_ansi: bool,
) -> Response {
    with_session(ctx, session_id, |session| Response::SearchResult {
        found: session.search_output_history(text, strip_ansi),
        running: session.is_running(),
    })
}
