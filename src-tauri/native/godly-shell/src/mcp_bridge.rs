//! Bridges the MCP pipe server events into the godly-shell async event bus.

use futures_channel::mpsc;
use godly_app_adapter::mcp_pipe::{McpEvent, start_mcp_server};
use crate::event_bus::{AsyncEvent, EventSender};

/// Start the MCP server and forward events to the shell event bus.
pub fn start(sender: EventSender) {
    let (tx, mut rx) = mpsc::unbounded();
    start_mcp_server(tx);

    std::thread::spawn(move || {
        // Poll the MCP event channel and forward to the event bus
        loop {
            match rx.try_next() {
                Ok(Some(event)) => {
                    log::debug!("MCP event: {event:?}");
                    // MCP events will be handled via the async event system
                    // For now, log them — full handling requires the action dispatch system
                }
                Ok(None) => break, // Channel closed
                Err(_) => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
        }
    });
}
