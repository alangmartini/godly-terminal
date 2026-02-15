use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::daemon_client::DaemonClient;
use crate::persistence::AutoSaveManager;
use crate::state::{AppState, ShellType, SplitView, Terminal, Workspace};

#[tauri::command]
pub fn create_workspace(
    name: String,
    folder_path: String,
    shell_type: Option<ShellType>,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<String, String> {
    let workspace_id = Uuid::new_v4().to_string();

    let workspace = Workspace {
        id: workspace_id.clone(),
        name,
        folder_path,
        tab_order: Vec::new(),
        shell_type: shell_type.unwrap_or_default(),
        worktree_mode: false,
        claude_code_mode: false,
    };

    state.add_workspace(workspace);
    auto_save.mark_dirty();

    Ok(workspace_id)
}

#[tauri::command]
pub fn delete_workspace(
    workspace_id: String,
    state: State<Arc<AppState>>,
    daemon: State<Arc<DaemonClient>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    // Close all terminals in the workspace via daemon
    let terminals = state.get_workspace_terminals(&workspace_id);
    for terminal in terminals {
        let request = godly_protocol::Request::CloseSession {
            session_id: terminal.id.clone(),
        };
        let _ = daemon.send_request(&request);
        state.remove_session_metadata(&terminal.id);
        state.remove_terminal(&terminal.id);
    }

    // Remove workspace
    state.remove_workspace(&workspace_id);
    auto_save.mark_dirty();

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
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    state.update_terminal_workspace(&terminal_id, target_workspace_id);
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn reorder_tabs(
    workspace_id: String,
    tab_order: Vec<String>,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    let mut workspaces = state.workspaces.write();
    if let Some(workspace) = workspaces.get_mut(&workspace_id) {
        workspace.tab_order = tab_order;
    }
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn set_split_view(
    workspace_id: String,
    left_terminal_id: String,
    right_terminal_id: String,
    direction: String,
    ratio: f64,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    state.set_split_view(
        &workspace_id,
        SplitView {
            left_terminal_id,
            right_terminal_id,
            direction,
            ratio,
        },
    );
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn clear_split_view(
    workspace_id: String,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    state.clear_split_view(&workspace_id);
    auto_save.mark_dirty();
    Ok(())
}

/// Returns the Agent workspace and its terminals for the MCP window to bootstrap
/// its state on load. This handles the race condition where the MCP window is
/// created but hasn't set up event listeners before `mcp-terminal-created` fires.
#[derive(serde::Serialize)]
pub struct McpState {
    pub workspace: Option<Workspace>,
    pub terminals: Vec<Terminal>,
}

#[tauri::command]
pub fn get_mcp_state(state: State<Arc<AppState>>) -> McpState {
    let workspace_id = state.mcp_workspace_id.read().clone();
    match workspace_id {
        Some(id) => {
            let workspace = state.get_workspace(&id);
            let terminals = state.get_workspace_terminals(&id);
            McpState {
                workspace,
                terminals,
            }
        }
        None => McpState {
            workspace: None,
            terminals: Vec::new(),
        },
    }
}
