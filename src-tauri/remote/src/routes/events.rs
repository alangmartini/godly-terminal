use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::Stream;

use crate::AppState;

/// Extract query param value by name.
fn query_param(uri: &axum::http::Uri, name: &str) -> Option<String> {
    uri.query().and_then(|q| {
        q.split('&').find_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            match (parts.next(), parts.next()) {
                (Some(k), Some(v)) if k == name => Some(v.to_string()),
                _ => None,
            }
        })
    })
}

/// GET /api/events
/// SSE endpoint for real-time prompt events.
/// Auth: accepts a one-time SSE ticket (preferred) or legacy api_key+device_token query params.
/// EventSource can't set custom headers, so secrets must NOT be in the URL for ticket-based auth.
pub async fn event_stream(
    State(state): State<AppState>,
    req: axum::extract::Request,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, (StatusCode, String)> {
    // Try ticket-based auth first (preferred — no secrets in URL)
    let ticket = query_param(req.uri(), "ticket");
    let authorized = if let Some(ref t) = ticket {
        state.sse_tickets.consume(t)
    } else {
        // Legacy: check api_key + device_token query params
        let api_key_ok = match &state.config.auth.api_key {
            None => true, // dev mode
            Some(expected) => {
                use subtle::ConstantTimeEq;
                query_param(req.uri(), "api_key")
                    .map(|k| k.as_bytes().ct_eq(expected.as_bytes()).into())
                    .unwrap_or(false)
            }
        };

        let device_ok = if !state.device_lock.is_locked() {
            true
        } else {
            query_param(req.uri(), "device_token")
                .map(|t| state.device_lock.check(&t))
                .unwrap_or(false)
        };

        api_key_ok && device_ok
    };

    if !authorized {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized".into()));
    }

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
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
