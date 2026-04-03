use crate::event_bus::{AsyncEvent, EventSender};
use godly_app_adapter::daemon_client::FrontendEventSink;
use std::sync::Arc;

pub struct ShellEventSink {
    sender: EventSender,
}

impl ShellEventSink {
    pub fn new(sender: EventSender) -> Self {
        Self { sender }
    }
}

impl FrontendEventSink for ShellEventSink {
    fn on_terminal_output(&self, session_id: &str) {
        self.sender.send(AsyncEvent::TerminalOutput {
            session_id: session_id.to_string(),
        });
    }

    fn on_session_closed(&self, session_id: &str, exit_code: Option<i64>) {
        self.sender.send(AsyncEvent::SessionClosed {
            session_id: session_id.to_string(),
            exit_code,
        });
    }

    fn on_process_changed(&self, session_id: &str, process_name: &str) {
        self.sender.send(AsyncEvent::ProcessChanged {
            session_id: session_id.to_string(),
            process_name: process_name.to_string(),
        });
    }

    fn on_grid_diff(&self, session_id: &str, diff_bytes: &[u8]) {
        self.sender.send(AsyncEvent::GridDiff {
            session_id: session_id.to_string(),
            diff_bytes: diff_bytes.to_vec(),
        });
    }

    fn on_bell(&self, session_id: &str) {
        self.sender.send(AsyncEvent::Bell {
            session_id: session_id.to_string(),
        });
    }
}
