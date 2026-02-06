use std::sync::Arc;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::persistence::AutoSaveManager;
use crate::pty::PtySession;
use crate::state::{AppState, ShellType, Terminal};

#[tauri::command]
pub fn create_terminal(
    workspace_id: String,
    cwd_override: Option<String>,
    shell_type_override: Option<ShellType>,
    id_override: Option<String>,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
    app_handle: AppHandle,
) -> Result<String, String> {
    // Use provided ID (for restoring terminals) or generate a new one
    let terminal_id = id_override.unwrap_or_else(|| Uuid::new_v4().to_string());

    // Get workspace info
    let workspace = state.get_workspace(&workspace_id);

    // Determine working directory: cwd_override > workspace folder
    let working_dir = cwd_override.or_else(|| workspace.as_ref().map(|w| w.folder_path.clone()));

    // Determine shell type: shell_type_override > workspace shell type > default
    let shell_type = shell_type_override
        .or_else(|| workspace.as_ref().map(|w| w.shell_type.clone()))
        .unwrap_or_default();

    // Determine initial process name based on shell type
    let process_name = match &shell_type {
        ShellType::Windows => String::from("powershell"),
        ShellType::Wsl { distribution } => {
            distribution.clone().unwrap_or_else(|| String::from("wsl"))
        }
    };

    // Create PTY session
    let session = PtySession::new(terminal_id.clone(), working_dir, shell_type, app_handle)?;

    // Store session
    state.add_pty_session(terminal_id.clone(), session);

    // Create terminal record
    let terminal = Terminal {
        id: terminal_id.clone(),
        workspace_id,
        name: String::from("Terminal"),
        process_name,
    };
    state.add_terminal(terminal);

    // Mark state as dirty for auto-save
    auto_save.mark_dirty();

    Ok(terminal_id)
}

#[tauri::command]
pub fn close_terminal(
    terminal_id: String,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    // Close PTY session
    if let Some(session) = state.remove_pty_session(&terminal_id) {
        session.close();
    }

    // Remove terminal record
    state.remove_terminal(&terminal_id);

    // Mark state as dirty for auto-save
    auto_save.mark_dirty();

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
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    state.update_terminal_name(&terminal_id, name);
    auto_save.mark_dirty();
    Ok(())
}
