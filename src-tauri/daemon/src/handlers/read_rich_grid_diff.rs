use godly_protocol::Response;

use super::context::with_session;
use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str) -> Response {
    with_session(ctx, session_id, |session| {
        let diff = session.read_rich_grid_diff();
        Response::RichGridDiff { diff }
    })
}
