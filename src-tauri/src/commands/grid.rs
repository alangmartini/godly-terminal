use std::sync::Arc;
use tauri::State;

use crate::daemon_client::DaemonClient;

use godly_protocol::{Request, Response};
use godly_protocol::types::RichGridData;

/// Get a rich grid snapshot with per-cell attributes for Canvas2D rendering.
#[tauri::command]
pub fn get_grid_snapshot(
    terminal_id: String,
    daemon: State<Arc<DaemonClient>>,
) -> Result<RichGridData, String> {
    let request = Request::ReadRichGrid {
        session_id: terminal_id,
    };
    let response = daemon.send_request(&request)?;
    match response {
        Response::RichGrid { grid } => Ok(grid),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Get the terminal grid dimensions (rows, cols).
#[tauri::command]
pub fn get_grid_dimensions(
    terminal_id: String,
    daemon: State<Arc<DaemonClient>>,
) -> Result<(u16, u16), String> {
    // Use the plain ReadGrid which is cheaper than RichGrid
    let request = Request::ReadGrid {
        session_id: terminal_id,
    };
    let response = daemon.send_request(&request)?;
    match response {
        Response::Grid { grid } => Ok((grid.num_rows, grid.cols)),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Set the scrollback viewport offset for a terminal session.
/// offset=0 means live view, offset>0 scrolls into history.
#[tauri::command]
pub fn set_scrollback(
    terminal_id: String,
    offset: usize,
    daemon: State<Arc<DaemonClient>>,
) -> Result<(), String> {
    let request = Request::SetScrollback {
        session_id: terminal_id,
        offset,
    };
    let response = daemon.send_request(&request)?;
    match response {
        Response::Ok => Ok(()),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}

/// Get selected text from the terminal grid between two positions.
#[tauri::command]
pub fn get_grid_text(
    terminal_id: String,
    start_row: u16,
    start_col: u16,
    end_row: u16,
    end_col: u16,
    daemon: State<Arc<DaemonClient>>,
) -> Result<String, String> {
    let request = Request::ReadGridText {
        session_id: terminal_id,
        start_row,
        start_col,
        end_row,
        end_col,
    };
    let response = daemon.send_request(&request)?;
    match response {
        Response::GridText { text } => Ok(text),
        Response::Error { message } => Err(message),
        other => Err(format!("Unexpected response: {:?}", other)),
    }
}
