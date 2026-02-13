use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

use crate::state::{AppState, Layout, TerminalInfo};

const STORE_PATH: &str = "layout.json";
const LAYOUT_KEY: &str = "layout";

fn log_info(msg: &str) {
    eprintln!("[persistence] {}", msg);
}

fn log_error(msg: &str) {
    eprintln!("[persistence] ERROR: {}", msg);
}

fn build_terminal_infos(state: &AppState) -> Vec<TerminalInfo> {
    let terminals = state.terminals.read();
    let session_meta = state.session_metadata.read();
    terminals
        .values()
        .map(|t| {
            let meta = session_meta.get(&t.id);
            let shell_type = meta
                .map(|m| m.shell_type.clone())
                .unwrap_or_default();
            let cwd = meta.and_then(|m| m.cwd.clone());

            let worktree_path = meta.and_then(|m| m.worktree_path.clone());
            let worktree_branch = meta.and_then(|m| m.worktree_branch.clone());

            TerminalInfo {
                id: t.id.clone(),
                workspace_id: t.workspace_id.clone(),
                name: t.name.clone(),
                shell_type,
                cwd,
                worktree_path,
                worktree_branch,
            }
        })
        .collect()
}

#[tauri::command]
pub fn save_layout(app_handle: AppHandle, state: State<Arc<AppState>>) -> Result<(), String> {
    log_info("Saving layout...");

    let store = app_handle
        .store(STORE_PATH)
        .map_err(|e| {
            let msg = format!("Failed to open store: {}", e);
            log_error(&msg);
            msg
        })?;

    let workspaces = state.get_all_workspaces();
    let terminals = build_terminal_infos(&state);
    let active_workspace_id = state.active_workspace_id.read().clone();
    let split_views = state.get_all_split_views();

    let layout = Layout {
        workspaces,
        terminals,
        active_workspace_id,
        split_views,
    };

    let json_value = serde_json::to_value(&layout)
        .map_err(|e| {
            let msg = format!("Failed to serialize layout: {}", e);
            log_error(&msg);
            msg
        })?;

    store.set(LAYOUT_KEY, json_value);
    store.save().map_err(|e| {
        let msg = format!("Failed to save store: {}", e);
        log_error(&msg);
        msg
    })?;

    log_info(&format!(
        "Layout saved: {} workspaces, {} terminals",
        layout.workspaces.len(),
        layout.terminals.len()
    ));

    Ok(())
}

#[tauri::command]
pub fn load_layout(app_handle: AppHandle, state: State<Arc<AppState>>) -> Result<Layout, String> {
    log_info("Loading layout...");

    let store = app_handle
        .store(STORE_PATH)
        .map_err(|e| {
            let msg = format!("Failed to open store: {}", e);
            log_error(&msg);
            msg
        })?;

    match store.get(LAYOUT_KEY) {
        Some(value) => {
            let layout: Layout = serde_json::from_value(value.clone())
                .map_err(|e| {
                    let msg = format!("Failed to parse layout: {}", e);
                    log_error(&msg);
                    msg
                })?;

            log_info(&format!(
                "Layout loaded: {} workspaces, {} terminals",
                layout.workspaces.len(),
                layout.terminals.len()
            ));

            // Restore workspaces to backend state so create_terminal can find them
            for ws in &layout.workspaces {
                log_info(&format!("Restoring workspace to backend state: {} ({})", ws.name, ws.id));
                state.add_workspace(crate::state::Workspace {
                    id: ws.id.clone(),
                    name: ws.name.clone(),
                    folder_path: ws.folder_path.clone(),
                    tab_order: ws.tab_order.clone(),
                    shell_type: ws.shell_type.clone(),
                    worktree_mode: ws.worktree_mode,
                    claude_code_mode: ws.claude_code_mode,
                });
            }

            // Set active workspace ID
            if let Some(active_id) = &layout.active_workspace_id {
                *state.active_workspace_id.write() = Some(active_id.clone());
            }

            Ok(layout)
        }
        None => {
            log_info("No saved layout found, using default");
            Ok(Layout::default())
        }
    }
}

pub fn save_on_exit(app_handle: &AppHandle, state: &Arc<AppState>) {
    log_info("Saving layout on exit...");

    let store = match app_handle.store(STORE_PATH) {
        Ok(s) => s,
        Err(e) => {
            log_error(&format!("Failed to open store on exit: {}", e));
            return;
        }
    };

    let workspaces = state.get_all_workspaces();
    let terminals = build_terminal_infos(state);
    let active_workspace_id = state.active_workspace_id.read().clone();
    let split_views = state.get_all_split_views();

    let layout = Layout {
        workspaces,
        terminals,
        active_workspace_id,
        split_views,
    };

    match serde_json::to_value(&layout) {
        Ok(json_value) => {
            store.set(LAYOUT_KEY, json_value);
            if let Err(e) = store.save() {
                log_error(&format!("Failed to save store on exit: {}", e));
            } else {
                log_info(&format!(
                    "Layout saved on exit: {} workspaces, {} terminals",
                    layout.workspaces.len(),
                    layout.terminals.len()
                ));
            }
        }
        Err(e) => {
            log_error(&format!("Failed to serialize layout on exit: {}", e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{SessionMetadata, ShellType, Terminal};

    #[test]
    fn test_custom_tab_name_survives_save_cycle() {
        // Bug: create_terminal hardcoded name="Terminal", so after autosave
        // the custom tab name was overwritten. This test verifies that when
        // a terminal with a custom name exists in backend state,
        // build_terminal_infos preserves that name for persistence.
        let state = AppState::new();

        state.add_terminal(Terminal {
            id: "term-1".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "My Custom Tab".to_string(),
            process_name: "powershell".to_string(),
        });

        state.add_session_metadata(
            "term-1".to_string(),
            SessionMetadata {
                shell_type: ShellType::Windows,
                cwd: Some("C:\\Projects".to_string()),
                worktree_path: None,
                worktree_branch: None,
            },
        );

        let infos = build_terminal_infos(&state);

        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "My Custom Tab");

        // Verify the name survives JSON round-trip (save → load cycle)
        let json = serde_json::to_string(&infos[0]).unwrap();
        let restored: TerminalInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.name, "My Custom Tab");
    }

    #[test]
    fn test_default_terminal_name_when_no_override() {
        // When no name_override is provided, create_terminal sets "Terminal".
        // Verify this default also persists correctly.
        let state = AppState::new();

        state.add_terminal(Terminal {
            id: "term-1".to_string(),
            workspace_id: "ws-1".to_string(),
            name: "Terminal".to_string(),
            process_name: "powershell".to_string(),
        });

        let infos = build_terminal_infos(&state);

        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].name, "Terminal");
    }
}

/// Save layout from a background thread context (not a command)
pub fn save_layout_internal(app_handle: &AppHandle, state: &Arc<AppState>) -> Result<(), String> {
    let store = app_handle
        .store(STORE_PATH)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let workspaces = state.get_all_workspaces();
    let terminals = build_terminal_infos(state);
    let active_workspace_id = state.active_workspace_id.read().clone();
    let split_views = state.get_all_split_views();

    let layout = Layout {
        workspaces,
        terminals,
        active_workspace_id,
        split_views,
    };

    let json_value = serde_json::to_value(&layout)
        .map_err(|e| format!("Failed to serialize layout: {}", e))?;

    store.set(LAYOUT_KEY, json_value);
    store.save().map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}
