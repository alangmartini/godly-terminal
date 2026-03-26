use std::sync::Arc;

use futures_channel::mpsc;

use godly_app_adapter::daemon_client::FrontendEventSink;

/// Events forwarded from the daemon bridge I/O thread to the Iced app.
#[derive(Debug, Clone)]
pub enum DaemonEventMsg {
    /// Terminal produced output — grid needs refresh.
    TerminalOutput { session_id: String },
    /// Terminal session closed.
    SessionClosed {
        session_id: String,
        exit_code: Option<i64>,
    },
    /// Process name changed (e.g., shell -> vim).
    ProcessChanged {
        session_id: String,
        process_name: String,
    },
    /// Bell character received.
    Bell { session_id: String },
    /// Heartbeat — keeps the Iced event loop alive when the window is minimized.
    /// Sent by a dedicated background thread, not by Iced subscriptions (which
    /// stop being polled when the window is invisible).
    Heartbeat,
}

/// Spawn a background thread that sends `DaemonEventMsg::Heartbeat` every second
/// through the given channel. This keeps the Iced event loop alive even when
/// the window is minimized and Iced stops polling its own subscriptions.
pub fn spawn_heartbeat_thread(sender: mpsc::UnboundedSender<DaemonEventMsg>) {
    std::thread::Builder::new()
        .name("iced-heartbeat".into())
        .spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
            if sender.unbounded_send(DaemonEventMsg::Heartbeat).is_err() {
                break; // Channel closed, app shutting down
            }
        })
        .expect("Failed to spawn heartbeat thread");
}

/// Event sink that sends daemon events through an mpsc channel to iced.
///
/// Implements `FrontendEventSink` so it can be handed to the bridge I/O thread.
/// Events are forwarded as `DaemonEventMsg` values through the unbounded sender.
pub struct ChannelEventSink {
    sender: mpsc::UnboundedSender<DaemonEventMsg>,
}

impl ChannelEventSink {
    pub fn new(sender: mpsc::UnboundedSender<DaemonEventMsg>) -> Self {
        Self { sender }
    }
}

impl FrontendEventSink for ChannelEventSink {
    fn on_terminal_output(&self, session_id: &str) {
        let _ = self.sender.unbounded_send(DaemonEventMsg::TerminalOutput {
            session_id: session_id.to_string(),
        });
    }

    fn on_session_closed(&self, session_id: &str, exit_code: Option<i64>) {
        let _ = self.sender.unbounded_send(DaemonEventMsg::SessionClosed {
            session_id: session_id.to_string(),
            exit_code,
        });
    }

    fn on_process_changed(&self, session_id: &str, process_name: &str) {
        let _ = self.sender.unbounded_send(DaemonEventMsg::ProcessChanged {
            session_id: session_id.to_string(),
            process_name: process_name.to_string(),
        });
    }

    fn on_grid_diff(&self, session_id: &str, _diff_bytes: &[u8]) {
        // Grid diffs trigger a terminal output event so the app fetches a fresh snapshot.
        let _ = self.sender.unbounded_send(DaemonEventMsg::TerminalOutput {
            session_id: session_id.to_string(),
        });
    }

    fn on_bell(&self, session_id: &str) {
        let _ = self.sender.unbounded_send(DaemonEventMsg::Bell {
            session_id: session_id.to_string(),
        });
    }
}

/// Creates an iced Subscription that streams DaemonEventMsg values from a channel receiver.
///
/// The receiver is wrapped in an `Arc<parking_lot::Mutex<Option<...>>>`. When the
/// subscription stream is dropped (e.g., Iced recreates it after dormancy), the
/// receiver is **put back** so the next `stream()` call can take it again. This
/// prevents the subscription from permanently dying after minimize/restore.
pub fn daemon_events(
    receiver: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<DaemonEventMsg>>>>,
) -> iced::Subscription<DaemonEventMsg> {
    use iced::advanced::subscription::{self, EventStream, Hasher, Recipe};
    use std::hash::Hash;

    struct DaemonEventRecipe {
        receiver: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<DaemonEventMsg>>>>,
    }

    impl Recipe for DaemonEventRecipe {
        type Output = DaemonEventMsg;

        fn hash(&self, state: &mut Hasher) {
            std::any::TypeId::of::<Self>().hash(state);
        }

        fn stream(
            self: Box<Self>,
            _input: EventStream,
        ) -> futures::stream::BoxStream<'static, Self::Output> {
            let rx = self.receiver.lock().take();
            if let Some(rx) = rx {
                // Wrap the receiver in a stream that puts it back on drop.
                Box::pin(RestoringStream {
                    inner: rx,
                    storage: self.receiver,
                })
            } else {
                // Receiver already taken by another active stream — wait for it.
                Box::pin(futures::stream::pending())
            }
        }
    }

    subscription::from_recipe(DaemonEventRecipe { receiver })
}

/// A stream wrapper that returns the inner receiver to shared storage on drop.
/// This allows the subscription to be recreated after Iced drops it (e.g., during
/// window minimize/restore dormancy cycles).
struct RestoringStream<T> {
    inner: mpsc::UnboundedReceiver<T>,
    storage: Arc<parking_lot::Mutex<Option<mpsc::UnboundedReceiver<T>>>>,
}

impl<T> futures::Stream for RestoringStream<T> {
    type Item = T;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl<T> Drop for RestoringStream<T> {
    fn drop(&mut self) {
        // Put the receiver back so the subscription can be recreated.
        // Safety: we need to move `inner` out, but we're in drop so self is
        // going away. Use a dummy receiver via a fresh channel.
        let (_, dummy) = mpsc::unbounded();
        let real = std::mem::replace(&mut self.inner, dummy);
        *self.storage.lock() = Some(real);
    }
}

/// Creates an iced Subscription that streams McpEvent values from a channel receiver.
///
/// Uses the same restoring-stream pattern as `daemon_events`.
pub fn mcp_events(
    receiver: Arc<
        parking_lot::Mutex<Option<mpsc::UnboundedReceiver<godly_app_adapter::mcp_pipe::McpEvent>>>,
    >,
) -> iced::Subscription<godly_app_adapter::mcp_pipe::McpEvent> {
    use iced::advanced::subscription::{self, EventStream, Hasher, Recipe};
    use std::hash::Hash;

    struct McpEventRecipe {
        receiver: Arc<
            parking_lot::Mutex<
                Option<mpsc::UnboundedReceiver<godly_app_adapter::mcp_pipe::McpEvent>>,
            >,
        >,
    }

    impl Recipe for McpEventRecipe {
        type Output = godly_app_adapter::mcp_pipe::McpEvent;

        fn hash(&self, state: &mut Hasher) {
            std::any::TypeId::of::<Self>().hash(state);
        }

        fn stream(
            self: Box<Self>,
            _input: EventStream,
        ) -> futures::stream::BoxStream<'static, Self::Output> {
            let rx = self.receiver.lock().take();
            if let Some(rx) = rx {
                Box::pin(RestoringStream {
                    inner: rx,
                    storage: self.receiver,
                })
            } else {
                Box::pin(futures::stream::pending())
            }
        }
    }

    subscription::from_recipe(McpEventRecipe { receiver })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_event_sink_sends_output() {
        let (tx, mut rx) = mpsc::unbounded();
        let sink = ChannelEventSink::new(tx);

        sink.on_terminal_output("sess1");

        match rx.try_recv() {
            Ok(DaemonEventMsg::TerminalOutput { session_id }) => {
                assert_eq!(session_id, "sess1");
            }
            other => panic!("Expected TerminalOutput, got: {:?}", other),
        }
    }

    #[test]
    fn channel_event_sink_sends_session_closed() {
        let (tx, mut rx) = mpsc::unbounded();
        let sink = ChannelEventSink::new(tx);

        sink.on_session_closed("sess2", Some(0));

        match rx.try_recv() {
            Ok(DaemonEventMsg::SessionClosed {
                session_id,
                exit_code,
            }) => {
                assert_eq!(session_id, "sess2");
                assert_eq!(exit_code, Some(0));
            }
            other => panic!("Expected SessionClosed, got: {:?}", other),
        }
    }

    #[test]
    fn channel_event_sink_sends_process_changed() {
        let (tx, mut rx) = mpsc::unbounded();
        let sink = ChannelEventSink::new(tx);

        sink.on_process_changed("sess3", "vim");

        match rx.try_recv() {
            Ok(DaemonEventMsg::ProcessChanged {
                session_id,
                process_name,
            }) => {
                assert_eq!(session_id, "sess3");
                assert_eq!(process_name, "vim");
            }
            other => panic!("Expected ProcessChanged, got: {:?}", other),
        }
    }

    #[test]
    fn channel_event_sink_sends_bell() {
        let (tx, mut rx) = mpsc::unbounded();
        let sink = ChannelEventSink::new(tx);

        sink.on_bell("sess4");

        match rx.try_recv() {
            Ok(DaemonEventMsg::Bell { session_id }) => {
                assert_eq!(session_id, "sess4");
            }
            other => panic!("Expected Bell, got: {:?}", other),
        }
    }

    #[test]
    fn channel_event_sink_grid_diff_becomes_output() {
        let (tx, mut rx) = mpsc::unbounded();
        let sink = ChannelEventSink::new(tx);

        sink.on_grid_diff("sess5", &[1, 2, 3]);

        match rx.try_recv() {
            Ok(DaemonEventMsg::TerminalOutput { session_id }) => {
                assert_eq!(session_id, "sess5");
            }
            other => panic!("Expected TerminalOutput from grid_diff, got: {:?}", other),
        }
    }
}
