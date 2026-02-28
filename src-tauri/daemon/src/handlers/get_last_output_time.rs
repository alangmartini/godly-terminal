use godly_protocol::Response;

use super::context::with_session;
use super::HandlerContext;

pub async fn handle(ctx: &HandlerContext, session_id: &str) -> Response {
    with_session(ctx, session_id, |session| Response::LastOutputTime {
        epoch_ms: session.last_output_epoch_ms(),
        running: session.is_running(),
        exit_code: session.exit_code(),
        input_expected: Some(session.is_likely_waiting_for_input()),
    })
}
