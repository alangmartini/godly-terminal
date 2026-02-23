use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct SseTicketResponse {
    pub ticket: String,
}

/// POST /api/sse-ticket — Get a one-time ticket for SSE connection.
/// Requires full authentication (API key + device token).
/// The ticket is valid for 30 seconds and can only be used once.
/// This eliminates the need to pass API key or device token in the SSE URL.
pub async fn create_sse_ticket(
    State(state): State<AppState>,
) -> Json<SseTicketResponse> {
    let ticket = state.sse_tickets.create();
    Json(SseTicketResponse { ticket })
}
