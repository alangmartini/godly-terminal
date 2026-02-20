use axum::extract::ws::{Message, WebSocket};
use axum::extract::{Path, State, WebSocketUpgrade};
use axum::response::Response;
use futures::stream::StreamExt;
use futures::SinkExt;
use serde::{Deserialize, Serialize};

use godly_protocol::{Request, Response as DaemonResponse};

use crate::daemon_client::async_request;
use crate::AppState;

/// Server → Client messages
#[derive(Serialize)]
#[serde(tag = "type")]
enum WsServerMessage {
    #[serde(rename = "grid")]
    Grid { rows: Vec<String>, cursor_row: u16, cursor_col: u16 },
    #[serde(rename = "session_closed")]
    SessionClosed { session_id: String, exit_code: Option<i64> },
    #[serde(rename = "error")]
    Error { message: String },
}

/// Client → Server messages
#[derive(Deserialize)]
#[serde(tag = "type")]
enum WsClientMessage {
    #[serde(rename = "write")]
    Write { data: String },
    #[serde(rename = "get_grid")]
    GetGrid,
}

pub async fn ws_upgrade(
    State(state): State<AppState>,
    Path(id): Path<String>,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(state, id, socket))
}

async fn handle_ws(state: AppState, session_id: String, socket: WebSocket) {
    let (mut sender, mut receiver) = socket.split();

    // Background task: poll grid and send updates
    let daemon = state.daemon.clone();
    let sid = session_id.clone();
    let grid_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
        let mut last_rows: Option<Vec<String>> = None;

        loop {
            interval.tick().await;

            let resp = async_request(
                &daemon,
                Request::ReadGrid {
                    session_id: sid.clone(),
                },
            )
            .await;

            let msg = match resp {
                Ok(DaemonResponse::Grid { grid }) => {
                    // Only send if changed
                    if last_rows.as_ref() == Some(&grid.rows) {
                        continue;
                    }
                    last_rows = Some(grid.rows.clone());
                    WsServerMessage::Grid {
                        rows: grid.rows,
                        cursor_row: grid.cursor_row,
                        cursor_col: grid.cursor_col,
                    }
                }
                Ok(DaemonResponse::Error { message }) => {
                    if message.contains("not found") {
                        WsServerMessage::SessionClosed {
                            session_id: sid.clone(),
                            exit_code: None,
                        }
                    } else {
                        WsServerMessage::Error { message }
                    }
                }
                Err(e) => WsServerMessage::Error {
                    message: e.to_string(),
                },
                _ => continue,
            };

            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(_) => continue,
            };

            if sender.send(Message::Text(json.into())).await.is_err() {
                break;
            }

            // If session closed, stop
            if matches!(msg, WsServerMessage::SessionClosed { .. }) {
                break;
            }
        }
    });

    // Handle incoming messages from client
    let daemon2 = state.daemon.clone();
    let sid2 = session_id.clone();
    while let Some(Ok(msg)) = receiver.next().await {
        let text = match msg {
            Message::Text(t) => t.to_string(),
            Message::Close(_) => break,
            _ => continue,
        };

        let client_msg: WsClientMessage = match serde_json::from_str(&text) {
            Ok(m) => m,
            Err(_) => continue,
        };

        match client_msg {
            WsClientMessage::Write { data } => {
                let converted = data.replace("\r\n", "\r").replace('\n', "\r");
                let _ = async_request(
                    &daemon2,
                    Request::Write {
                        session_id: sid2.clone(),
                        data: converted.into_bytes(),
                    },
                )
                .await;
            }
            WsClientMessage::GetGrid => {
                // Grid is already being polled and sent automatically.
                // This is a manual trigger — the next poll tick will pick it up.
            }
        }
    }

    grid_task.abort();
}
