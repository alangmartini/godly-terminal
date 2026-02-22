use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;

use crate::AppState;

/// GET /api/events
/// SSE endpoint for real-time prompt events.
/// Streams: prompt_detected, prompt_resolved, heartbeat.
pub async fn event_stream(
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, (StatusCode, String)> {
    let mut rx = state.event_pump.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(sse_event) => {
                    let (event_type, data) = match &sse_event {
                        crate::event_pump::SseEvent::PromptDetected { .. } => {
                            ("prompt_detected", serde_json::to_string(&sse_event).unwrap_or_default())
                        }
                        crate::event_pump::SseEvent::PromptResolved { .. } => {
                            ("prompt_resolved", serde_json::to_string(&sse_event).unwrap_or_default())
                        }
                        crate::event_pump::SseEvent::Heartbeat { .. } => {
                            ("heartbeat", serde_json::to_string(&sse_event).unwrap_or_default())
                        }
                    };

                    yield Ok(Event::default().event(event_type).data(data));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("SSE client lagged, missed {} events", n);
                    // Continue receiving
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
