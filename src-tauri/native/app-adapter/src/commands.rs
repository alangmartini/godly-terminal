use godly_protocol::types::{RichGridData, SessionInfo};
use godly_protocol::{Request, Response, ShellType};

use crate::daemon_client::NativeDaemonClient;

/// Create a terminal session and attach to it.
pub fn create_terminal(
    client: &NativeDaemonClient,
    id: &str,
    shell_type: ShellType,
    cwd: Option<&str>,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    let response = client.send_request(&Request::CreateSession {
        id: id.to_string(),
        shell_type,
        cwd: cwd.map(|s| s.to_string()),
        rows,
        cols,
        env: None,
    })?;

    match response {
        Response::SessionCreated { .. } => {}
        Response::Error { message } => return Err(message),
        other => return Err(format!("Unexpected create response: {:?}", other)),
    }

    // Attach to start receiving events
    attach_session(client, id)?;

    Ok(())
}

/// Close a terminal session.
pub fn close_terminal(client: &NativeDaemonClient, session_id: &str) -> Result<(), String> {
    let response = client.send_request(&Request::CloseSession {
        session_id: session_id.to_string(),
    })?;

    match response {
        Response::Ok => {}
        Response::Error { message } => {
            log::warn!("Close session error: {}", message);
        }
        _ => {}
    }

    client.track_detach(session_id);
    Ok(())
}

/// Attach to a session to start receiving events.
pub fn attach_session(client: &NativeDaemonClient, session_id: &str) -> Result<(), String> {
    let response = client.send_request(&Request::Attach {
        session_id: session_id.to_string(),
    })?;

    match response {
        Response::Ok | Response::Buffer { .. } => {}
        Response::Error { message } => return Err(format!("Attach failed: {}", message)),
        other => return Err(format!("Unexpected attach response: {:?}", other)),
    }

    client.track_attach(session_id.to_string());
    Ok(())
}

/// Detach from a session.
pub fn detach_session(client: &NativeDaemonClient, session_id: &str) -> Result<(), String> {
    let response = client.send_request(&Request::Detach {
        session_id: session_id.to_string(),
    })?;

    match response {
        Response::Ok => {}
        Response::Error { message } => return Err(format!("Detach failed: {}", message)),
        other => return Err(format!("Unexpected detach response: {:?}", other)),
    }

    client.track_detach(session_id);
    Ok(())
}

/// Write input to a terminal session (fire-and-forget for low latency).
/// Converts `\n` to `\r` for PTY compatibility.
pub fn write_to_terminal(
    client: &NativeDaemonClient,
    session_id: &str,
    data: &[u8],
) -> Result<(), String> {
    client.send_fire_and_forget(&Request::Write {
        session_id: session_id.to_string(),
        data: data.to_vec(),
    })
}

/// Resize a terminal session (fire-and-forget).
pub fn resize_terminal(
    client: &NativeDaemonClient,
    session_id: &str,
    rows: u16,
    cols: u16,
) -> Result<(), String> {
    client.send_fire_and_forget(&Request::Resize {
        session_id: session_id.to_string(),
        rows,
        cols,
    })
}

/// Fetch the full grid snapshot from the daemon.
pub fn get_grid_snapshot(
    client: &NativeDaemonClient,
    session_id: &str,
) -> Result<RichGridData, String> {
    let response = client.send_request(&Request::ReadRichGrid {
        session_id: session_id.to_string(),
    })?;

    match response {
        Response::RichGrid { grid } => Ok(grid),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected grid response: {:?}", other)),
    }
}

/// Set the scrollback offset.
pub fn set_scrollback(
    client: &NativeDaemonClient,
    session_id: &str,
    offset: usize,
) -> Result<(), String> {
    let response = client.send_request(&Request::SetScrollback {
        session_id: session_id.to_string(),
        offset,
    })?;

    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected scrollback response: {:?}", other)),
    }
}

/// Set scrollback offset and fetch the grid in one round-trip.
pub fn scroll_and_get_snapshot(
    client: &NativeDaemonClient,
    session_id: &str,
    offset: usize,
) -> Result<RichGridData, String> {
    let response = client.send_request(&Request::ScrollAndReadRichGrid {
        session_id: session_id.to_string(),
        offset,
    })?;

    match response {
        Response::RichGrid { grid } => Ok(grid),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected scroll+grid response: {:?}", other)),
    }
}

/// List all live sessions on the daemon.
///
/// Used for session recovery on app restart — discovers sessions
/// that survived from a previous app instance.
pub fn list_sessions(client: &NativeDaemonClient) -> Result<Vec<SessionInfo>, String> {
    let response = client.send_request(&Request::ListSessions)?;
    match response {
        Response::SessionList { sessions } => Ok(sessions),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected list response: {:?}", other)),
    }
}

/// Detach from all sessions for clean shutdown.
///
/// Errors are logged but not propagated — best-effort cleanup.
pub fn detach_all_sessions(
    client: &NativeDaemonClient,
    session_ids: &[String],
) -> Result<(), String> {
    for id in session_ids {
        if let Err(e) = detach_session(client, id) {
            log::warn!("Failed to detach session {}: {}", id, e);
        }
    }
    Ok(())
}
