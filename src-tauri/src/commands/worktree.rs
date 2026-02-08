use std::sync::Arc;
use tauri::State;

use crate::persistence::AutoSaveManager;
use crate::state::AppState;
use crate::worktree;

#[tauri::command]
pub fn toggle_worktree_mode(
    workspace_id: String,
    enabled: bool,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    state.update_workspace_worktree_mode(&workspace_id, enabled);
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn toggle_claude_code_mode(
    workspace_id: String,
    enabled: bool,
    state: State<Arc<AppState>>,
    auto_save: State<Arc<AutoSaveManager>>,
) -> Result<(), String> {
    state.update_workspace_claude_code_mode(&workspace_id, enabled);
    auto_save.mark_dirty();
    Ok(())
}

#[tauri::command]
pub fn is_git_repo(folder_path: String) -> bool {
    worktree::is_git_repo(&folder_path)
}

#[tauri::command]
pub async fn list_worktrees(folder_path: String) -> Result<Vec<worktree::WorktreeInfo>, String> {
    tokio::task::spawn_blocking(move || {
        let repo_root = worktree::get_repo_root(&folder_path)?;
        worktree::list_worktrees(&repo_root)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn remove_worktree(
    folder_path: String,
    worktree_path: String,
    force: Option<bool>,
) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        let repo_root = worktree::get_repo_root(&folder_path)?;
        worktree::remove_worktree(&repo_root, &worktree_path, force.unwrap_or(false))
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

#[tauri::command]
pub async fn cleanup_all_worktrees(folder_path: String) -> Result<u32, String> {
    tokio::task::spawn_blocking(move || {
        let repo_root = worktree::get_repo_root(&folder_path)?;
        worktree::cleanup_all_worktrees(&repo_root)
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}
