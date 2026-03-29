use winit::event_loop::EventLoopProxy;

/// Events that can arrive from background threads (daemon, MCP, timers).
#[derive(Debug)]
pub enum AsyncEvent {
    TerminalOutput { session_id: String },
    SessionClosed { session_id: String, exit_code: Option<i64> },
    ProcessChanged { session_id: String, process_name: String },
    GridDiff { session_id: String, diff_bytes: Vec<u8> },
    Bell { session_id: String },
    Heartbeat,
}

/// Wakes the winit event loop from any thread.
#[derive(Clone)]
pub struct EventSender {
    proxy: EventLoopProxy<AsyncEvent>,
}

impl EventSender {
    pub fn new(proxy: EventLoopProxy<AsyncEvent>) -> Self {
        Self { proxy }
    }

    pub fn send(&self, event: AsyncEvent) {
        let _ = self.proxy.send_event(event);
    }
}
