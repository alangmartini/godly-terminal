use godly_protocol::Response;

use super::context::with_session;
use super::HandlerContext;

pub async fn handle(
    ctx: &HandlerContext,
    session_id: &str,
    start_row: i32,
    start_col: u16,
    end_row: i32,
    end_col: u16,
    scrollback_offset: usize,
) -> Response {
    with_session(ctx, session_id, |session| {
        let text = session.read_grid_text(start_row, start_col, end_row, end_col, scrollback_offset);
        Response::GridText { text }
    })
}
