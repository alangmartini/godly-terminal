use godly_protocol::Response;

use super::context::with_session;
use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str, offset: usize) -> Response {
    with_session(ctx, session_id, |session| {
        session.set_scrollback(offset);
        let grid = session.read_rich_grid();
        Response::RichGrid { grid }
    })
}
