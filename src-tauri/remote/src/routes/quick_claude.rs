use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use godly_protocol::{Request, Response, ShellType};

use crate::daemon_client::async_request;
use crate::AppState;

#[derive(Deserialize)]
pub struct QuickClaudeRequest {
    pub prompt: String,
    /// Session ID to reuse. If not provided, creates a new session.
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    /// Start permission monitor after sending prompt.
    #[serde(default)]
    pub monitor: bool,
}

#[derive(Serialize)]
pub struct QuickClaudeResponse {
    pub session_id: String,
    pub status: &'static str,
}

pub async fn quick_claude(
    State(state): State<AppState>,
    Json(body): Json<QuickClaudeRequest>,
) -> Result<(StatusCode, Json<QuickClaudeResponse>), (StatusCode, String)> {
    let session_id = match body.session_id {
        Some(id) => id,
        None => {
            // Create a new session
            let id = uuid::Uuid::new_v4().to_string();
            let mut env_vars = HashMap::new();
            env_vars.insert("GODLY_SESSION_ID".to_string(), id.clone());

            let create_req = Request::CreateSession {
                id: id.clone(),
                shell_type: ShellType::Windows,
                cwd: body.cwd.clone(),
                rows: 24,
                cols: 80,
                env: Some(env_vars),
            };

            let resp = async_request(&state.daemon, create_req)
                .await
                .map_err(|e| (StatusCode::BAD_GATEWAY, e))?;

            match resp {
                Response::SessionCreated { .. } => {}
                Response::Error { message } => {
                    return Err((StatusCode::BAD_REQUEST, message));
                }
                other => {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        format!("Unexpected response: {:?}", other),
                    ));
                }
            }

            // Attach
            let _ = async_request(
                &state.daemon,
                Request::Attach {
                    session_id: id.clone(),
                },
            )
            .await;

            id
        }
    };

    let sid = session_id.clone();
    let prompt = body.prompt.clone();
    let monitor = body.monitor;
    let daemon = Arc::clone(&state.daemon);
    let monitor_state = if monitor {
        Some(state.clone())
    } else {
        None
    };

    // Run the Quick Claude sequence in the background
    tokio::spawn(async move {
        if let Err(e) = run_quick_claude_sequence(&daemon, &sid, &prompt).await {
            tracing::error!("Quick Claude sequence failed for {}: {}", sid, e);
        }

        // Optionally start monitor
        if let Some(state) = monitor_state {
            crate::monitor::start_monitor(state, sid);
        }
    });

    Ok((
        StatusCode::ACCEPTED,
        Json(QuickClaudeResponse {
            session_id,
            status: "started",
        }),
    ))
}

/// Mirrors `quick_claude_background` from commands/terminal.rs
async fn run_quick_claude_sequence(
    daemon: &crate::daemon_client::DaemonClient,
    session_id: &str,
    prompt: &str,
) -> Result<(), String> {
    // Step 1: Wait for shell to be ready (idle for 500ms, timeout 5s)
    let shell_ready = poll_idle(daemon, session_id, 500, 5_000).await;
    if !shell_ready {
        tracing::warn!("Shell did not become idle in time, writing claude command anyway");
    }

    // Step 2: Write claude command
    async_request(
        daemon,
        Request::Write {
            session_id: session_id.to_string(),
            data: "claude --dangerously-skip-permissions\r".as_bytes().to_vec(),
        },
    )
    .await?;

    // Step 3: Wait for Claude to be ready (idle for 2000ms, timeout 60s)
    let claude_ready = poll_idle(daemon, session_id, 2_000, 60_000).await;
    if !claude_ready {
        tracing::warn!("Claude did not become idle within timeout, writing prompt anyway");
    }

    // Step 4: Small delay
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    // Step 5: Write the prompt (convert \n → \r for PTY)
    let prompt_data = format!("{}\r", prompt.replace("\r\n", "\r").replace('\n', "\r"));
    async_request(
        daemon,
        Request::Write {
            session_id: session_id.to_string(),
            data: prompt_data.into_bytes(),
        },
    )
    .await?;

    tracing::info!("Quick Claude prompt delivered to session {}", session_id);
    Ok(())
}

/// Async version of poll_idle from commands/terminal.rs
async fn poll_idle(
    daemon: &crate::daemon_client::DaemonClient,
    session_id: &str,
    idle_ms: u64,
    timeout_ms: u64,
) -> bool {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let poll_interval = std::time::Duration::from_millis((idle_ms / 4).min(500).max(50));

    loop {
        let resp = async_request(
            daemon,
            Request::GetLastOutputTime {
                session_id: session_id.to_string(),
            },
        )
        .await;

        match resp {
            Ok(Response::LastOutputTime { epoch_ms, running }) => {
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                let ago = now_ms.saturating_sub(epoch_ms);

                if ago >= idle_ms || !running {
                    return true;
                }
            }
            _ => return false,
        }

        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(poll_interval).await;
    }
}
