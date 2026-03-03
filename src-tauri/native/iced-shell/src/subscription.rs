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
/// The receiver is wrapped in an `Arc<parking_lot::Mutex<Option<...>>>` so it can be
/// taken exactly once by the subscription stream. Subsequent calls (from iced's
/// subscription deduplication) will produce an empty stream since the receiver is gone.
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
            if let Some(rx) = self.receiver.lock().take() {
                Box::pin(rx)
            } else {
                // Receiver already taken — return an empty stream that never completes.
                // This keeps the subscription alive (iced won't re-create it).
                Box::pin(futures::stream::pending())
            }
        }
    }

    subscription::from_recipe(DaemonEventRecipe { receiver })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_event_sink_sends_output() {
        let (tx, mut rx) = mpsc::unbounded();
        let sink = ChannelEventSink::new(tx);

        sink.on_terminal_output("sess1");

        match rx.try_next() {
            Ok(Some(DaemonEventMsg::TerminalOutput { session_id })) => {
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

        match rx.try_next() {
            Ok(Some(DaemonEventMsg::SessionClosed {
                session_id,
                exit_code,
            })) => {
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

        match rx.try_next() {
            Ok(Some(DaemonEventMsg::ProcessChanged {
                session_id,
                process_name,
            })) => {
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

        match rx.try_next() {
            Ok(Some(DaemonEventMsg::Bell { session_id })) => {
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

        match rx.try_next() {
            Ok(Some(DaemonEventMsg::TerminalOutput { session_id })) => {
                assert_eq!(session_id, "sess5");
            }
            other => panic!("Expected TerminalOutput from grid_diff, got: {:?}", other),
        }
    }
}
