use godly_protocol::{LayoutNode, SplitDirection};
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

use crate::daemon_client::DaemonClient;
use crate::persistence::AutoSaveManager;
#[allow(deprecated)]
use crate::state::{
    AiToolMode, AppState, ShellType, SplitView, Workspace, WorkspaceGitHubAuthPolicy,
};
use std::collections::HashSet;

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
        ai_tool_mode: AiToolMode::None,
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

fn is_valid_env_var_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

#[tauri::command]
pub fn set_workspace_github_auth_policy(
    workspace_id: String,
    policy: WorkspaceGitHubAuthPolicy,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    if state.get_workspace(&workspace_id).is_none() {
        return Err("Workspace not found".to_string());
    }

    if policy.rules.len() > 64 {
        return Err("GitHub auth policy supports at most 64 rules".to_string());
    }

    for rule in &policy.rules {
        if rule.pattern.trim().is_empty() {
            return Err("GitHub auth rule pattern must not be empty".to_string());
        }
        if !is_valid_env_var_name(&rule.token_env_var) {
            return Err(format!(
                "Invalid token_env_var '{}': use only letters, numbers, and underscores",
                rule.token_env_var
            ));
        }
    }

    state.set_workspace_github_auth_policy(&workspace_id, policy);
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn get_workspace_github_auth_policy(
    workspace_id: String,
    state: State<Arc<AppState>>,
) -> Result<WorkspaceGitHubAuthPolicy, String> {
    if state.get_workspace(&workspace_id).is_none() {
        return Err("Workspace not found".to_string());
    }
    Ok(state.get_workspace_github_auth_policy(&workspace_id))
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
    let terminals = state.terminals.read();
    let filtered: Vec<String> = tab_order
        .into_iter()
        .filter(|id| terminals.contains_key(id))
        .collect();
    drop(terminals);
    let mut workspaces = state.workspaces.write();
    if let Some(workspace) = workspaces.get_mut(&workspace_id) {
        workspace.tab_order = filtered;
    }
    auto_save.mark_dirty();
    Ok(())
}

#[allow(deprecated)]
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

#[tauri::command]
pub fn split_terminal(
    workspace_id: String,
    target_terminal_id: String,
    new_terminal_id: String,
    direction: SplitDirection,
    ratio: f64,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    let mut trees = state.layout_trees.write();
    let tree = trees.entry(workspace_id).or_insert_with(|| {
        // No tree yet — create one with the target as the root leaf
        LayoutNode::Leaf {
            terminal_id: target_terminal_id.clone(),
        }
    });
    if !tree.split_at(&target_terminal_id, &new_terminal_id, direction, ratio) {
        return Err(format!(
            "Terminal {} not found in layout tree",
            target_terminal_id
        ));
    }
    drop(trees);
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn unsplit_terminal(
    workspace_id: String,
    terminal_id: String,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    let mut trees = state.layout_trees.write();
    if let Some(tree) = trees.get_mut(&workspace_id) {
        // Check if this terminal is the root (only leaf)
        if let LayoutNode::Leaf {
            terminal_id: ref id,
        } = tree
        {
            if id == &terminal_id {
                // Last terminal — remove the tree entirely
                trees.remove(&workspace_id);
                drop(trees);
                auto_save.mark_dirty();
                return Ok(());
            }
        }
        match tree.remove_terminal(&terminal_id) {
            Some(_sibling) => {
                // If the tree collapsed to a single leaf, remove it
                if tree.count_leaves() <= 1 {
                    trees.remove(&workspace_id);
                }
            }
            None => {
                return Err(format!("Terminal {} not found in layout tree", terminal_id));
            }
        }
    } else {
        return Err(format!("No layout tree for workspace {}", workspace_id));
    }
    drop(trees);
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn get_layout_tree(workspace_id: String, state: State<Arc<AppState>>) -> Option<LayoutNode> {
    state.get_layout_tree(&workspace_id)
}

#[tauri::command]
pub fn swap_panes(
    workspace_id: String,
    terminal_id_a: String,
    terminal_id_b: String,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    let mut trees = state.layout_trees.write();
    if let Some(tree) = trees.get_mut(&workspace_id) {
        if !tree.swap_terminals(&terminal_id_a, &terminal_id_b) {
            return Err(format!(
                "One or both terminals ({}, {}) not found in layout tree",
                terminal_id_a, terminal_id_b
            ));
        }
    } else {
        return Err(format!("No layout tree for workspace {}", workspace_id));
    }
    drop(trees);
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn set_layout_tree(
    workspace_id: String,
    tree: LayoutNode,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    state.set_layout_tree_validated(&workspace_id, tree);
    auto_save.mark_dirty();
    Ok(())
}

/// Prune stale terminal IDs from layout trees, split views, zoomed panes,
/// tab orders, and active terminal ID. Called by the frontend after terminal
/// restoration is complete so the backend knows which IDs are live.
#[tauri::command]
pub fn prune_stale_terminal_ids(
    live_terminal_ids: Vec<String>,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    let live_ids: HashSet<String> = live_terminal_ids.into_iter().collect();
    eprintln!(
        "[workspace] prune_stale_terminal_ids: {} live terminals",
        live_ids.len()
    );
    state.prune_stale_ids(&live_ids);
    auto_save.mark_dirty();
    Ok(())
}
