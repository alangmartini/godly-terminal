use crate::daemon_client::FrontendEventSink;

/// Configuration for the native event loop.
pub struct EventLoopConfig {
    pub poll_interval_ms: u64,
}

impl Default for EventLoopConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: 1,
        }
    }
}

/// Start the event loop (stub — returns immediately in Phase 0).
pub fn start_event_loop<S: FrontendEventSink>(
    _sink: S,
    _config: EventLoopConfig,
) {
    log::info!("Native event loop stub — full implementation in Phase 1");
}
