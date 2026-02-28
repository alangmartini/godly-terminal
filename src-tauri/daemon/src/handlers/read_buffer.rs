use godly_protocol::Response;

use super::context::with_session;
use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str) -> Response {
    with_session(ctx, session_id, |session| {
        let data = session.read_output_history();
        Response::Buffer {
            session_id: session_id.to_string(),
            data,
        }
    })
}
