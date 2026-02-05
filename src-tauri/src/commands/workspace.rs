use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::state::{AppState, Workspace};

#[tauri::command]
pub fn create_workspace(
    name: String,
    folder_path: String,
    state: State<Arc<AppState>>,
) -> Result<String, String> {
    let workspace_id = Uuid::new_v4().to_string();

    let workspace = Workspace {
        id: workspace_id.clone(),
        name,
        folder_path,
        tab_order: Vec::new(),
    };

    state.add_workspace(workspace);

    Ok(workspace_id)
}

#[tauri::command]
pub fn delete_workspace(workspace_id: String, state: State<Arc<AppState>>) -> Result<(), String> {
    // Close all terminals in the workspace
    let terminals = state.get_workspace_terminals(&workspace_id);
    for terminal in terminals {
        if let Some(session) = state.remove_pty_session(&terminal.id) {
            session.close();
        }
        state.remove_terminal(&terminal.id);
    }

    // Remove workspace
    state.remove_workspace(&workspace_id);

    Ok(())
}

#[tauri::command]
pub fn get_workspaces(state: State<Arc<AppState>>) -> Vec<Workspace> {
    state.get_all_workspaces()
}

#[tauri::command]
pub fn move_tab_to_workspace(
    terminal_id: String,
    target_workspace_id: String,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    state.update_terminal_workspace(&terminal_id, target_workspace_id);
    Ok(())
}

#[tauri::command]
pub fn reorder_tabs(
    workspace_id: String,
    tab_order: Vec<String>,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    let mut workspaces = state.workspaces.write();
    if let Some(workspace) = workspaces.get_mut(&workspace_id) {
        workspace.tab_order = tab_order;
    }
    Ok(())
}
