use std::sync::Arc;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::pty::PtySession;
use crate::state::{AppState, Terminal};

#[tauri::command]
pub fn create_terminal(
    workspace_id: String,
    state: State<Arc<AppState>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    let terminal_id = Uuid::new_v4().to_string();

    // Get workspace folder path
    let working_dir = state.get_workspace(&workspace_id).map(|w| w.folder_path);

    // Create PTY session
    let session = PtySession::new(terminal_id.clone(), working_dir, app_handle)?;

    // Store session
    state.add_pty_session(terminal_id.clone(), session);

    // Create terminal record
    let terminal = Terminal {
        id: terminal_id.clone(),
        workspace_id,
        name: String::from("Terminal"),
        process_name: String::from("powershell"),
    };
    state.add_terminal(terminal);

    Ok(terminal_id)
}

#[tauri::command]
pub fn close_terminal(terminal_id: String, state: State<Arc<AppState>>) -> Result<(), String> {
    // Close PTY session
    if let Some(session) = state.remove_pty_session(&terminal_id) {
        session.close();
    }

    // Remove terminal record
    state.remove_terminal(&terminal_id);

    Ok(())
}

#[tauri::command]
pub fn write_to_terminal(
    terminal_id: String,
    data: String,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    let sessions = state.pty_sessions.read();
    let session = sessions
        .get(&terminal_id)
        .ok_or_else(|| format!("Terminal {} not found", terminal_id))?;

    session.write(data.as_bytes())
}

#[tauri::command]
pub fn resize_terminal(
    terminal_id: String,
    rows: u16,
    cols: u16,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    let sessions = state.pty_sessions.read();
    let session = sessions
        .get(&terminal_id)
        .ok_or_else(|| format!("Terminal {} not found", terminal_id))?;

    session.resize(rows, cols)
}

#[tauri::command]
pub fn rename_terminal(
    terminal_id: String,
    name: String,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    state.update_terminal_name(&terminal_id, name);
    Ok(())
}
