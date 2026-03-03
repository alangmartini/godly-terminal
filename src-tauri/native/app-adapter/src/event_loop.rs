use std::sync::Arc;

use crate::daemon_client::{FrontendEventSink, NativeDaemonClient};

/// Set up the bridge I/O thread on the daemon client.
///
/// This hands off the pipe reader/writer to a background thread that
/// handles all pipe I/O, dispatching events to the sink and routing
/// responses back to callers.
pub fn setup_bridge<S: FrontendEventSink>(
    client: &NativeDaemonClient,
    sink: Arc<S>,
) -> Result<(), String> {
    client.setup_bridge(sink)
}
