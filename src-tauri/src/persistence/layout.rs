use std::sync::Arc;
use godly_protocol::{LayoutNode, SplitDirection};
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

#[allow(deprecated)]
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

#[allow(deprecated)]
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
    let layout_trees = state.get_all_layout_trees();
    let window_state = state.window_state.read().clone();

    let layout = Layout {
        workspaces,
        terminals,
        active_workspace_id,
        split_views,
        layout_trees,
        window_state,
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

#[allow(deprecated)]
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
                    ai_tool_mode: ws.ai_tool_mode,
                });
            }

            // Set active workspace ID
            if let Some(active_id) = &layout.active_workspace_id {
                *state.active_workspace_id.write() = Some(active_id.clone());
            }

            // Restore window state to backend state
            if let Some(ws) = &layout.window_state {
                log_info(&format!(
                    "Restored window state: {}x{} at ({},{}) monitor={:?}",
                    ws.width, ws.height, ws.x, ws.y, ws.monitor_name
                ));
                *state.window_state.write() = layout.window_state.clone();
            }

            // Restore layout trees to backend state
            if !layout.layout_trees.is_empty() {
                for (ws_id, tree) in &layout.layout_trees {
                    state.set_layout_tree(ws_id, tree.clone());
                }
                log_info(&format!("Restored {} layout trees", layout.layout_trees.len()));
            } else if !layout.split_views.is_empty() {
                // Migrate old split_views to layout trees
                for (ws_id, sv) in &layout.split_views {
                    let direction = if sv.direction == "vertical" {
                        SplitDirection::Vertical
                    } else {
                        SplitDirection::Horizontal
                    };
                    let tree = LayoutNode::Split {
                        direction,
                        ratio: sv.ratio,
                        first: Box::new(LayoutNode::Leaf {
                            terminal_id: sv.left_terminal_id.clone(),
                        }),
                        second: Box::new(LayoutNode::Leaf {
                            terminal_id: sv.right_terminal_id.clone(),
                        }),
                    };
                    state.set_layout_tree(ws_id, tree);
                }
                log_info(&format!(
                    "Migrated {} split_views to layout trees",
                    layout.split_views.len()
                ));
            }

            Ok(layout)
        }
        None => {
            log_info("No saved layout found, using default");
            Ok(Layout::default())
        }
    }
}

/// Apply the saved window state to the window, validating that the target
/// monitor is still connected. Falls back to primary monitor if missing.
/// Clamps position to the visible area to prevent off-screen restoration.
#[tauri::command]
pub fn restore_window_state(
    window: tauri::WebviewWindow,
    state: State<Arc<AppState>>,
) -> Result<(), String> {
    let saved = state.window_state.read().clone();
    let saved = match saved {
        Some(s) => s,
        None => {
            log_info("No saved window state to restore");
            return Ok(());
        }
    };

    // Check if the saved monitor is still available
    let monitors = window.available_monitors().unwrap_or_default();
    let saved_monitor_available = saved.monitor_name.as_ref().map_or(false, |name| {
        monitors.iter().any(|m| m.name().map_or(false, |n| n == name))
    });

    if !saved_monitor_available {
        if let Some(ref name) = saved.monitor_name {
            log_info(&format!(
                "Saved monitor '{}' not found, falling back to primary monitor",
                name
            ));
        }
        // Fall back to primary — don't set position, let the OS place it
        if saved.maximized {
            let _ = window.maximize();
        }
        return Ok(());
    }

    // Find the target monitor's work area for clamping
    let target_monitor = monitors.iter().find(|m| {
        m.name().map_or(false, |n| {
            Some(n.to_string()) == saved.monitor_name
        })
    });

    let (mon_x, mon_y, mon_w, mon_h) = if let Some(m) = target_monitor {
        let pos = m.position();
        let size = m.size();
        (
            pos.x,
            pos.y,
            size.width as i32,
            size.height as i32,
        )
    } else {
        // Shouldn't reach here since we checked above, but fallback
        return Ok(());
    };

    // Clamp position so the window is at least partially visible
    let w = saved.width as i32;
    let h = saved.height as i32;
    let min_visible = 100; // at least 100px must be on screen
    let x = saved.x.max(mon_x - w + min_visible).min(mon_x + mon_w - min_visible);
    let y = saved.y.max(mon_y).min(mon_y + mon_h - min_visible);

    log_info(&format!(
        "Restoring window to ({},{}) {}x{} on monitor {:?}",
        x, y, saved.width, saved.height, saved.monitor_name
    ));

    let _ = window.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
    let _ = window.set_size(tauri::Size::Physical(tauri::PhysicalSize {
        width: saved.width,
        height: saved.height,
    }));

    if saved.maximized {
        let _ = window.maximize();
    }

    Ok(())
}

#[allow(deprecated)]
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
    let layout_trees = state.get_all_layout_trees();
    let window_state = state.window_state.read().clone();

    let layout = Layout {
        workspaces,
        terminals,
        active_workspace_id,
        split_views,
        layout_trees,
        window_state,
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
#[allow(deprecated)]
pub fn save_layout_internal(app_handle: &AppHandle, state: &Arc<AppState>) -> Result<(), String> {
    let store = app_handle
        .store(STORE_PATH)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let workspaces = state.get_all_workspaces();
    let terminals = build_terminal_infos(state);
    let active_workspace_id = state.active_workspace_id.read().clone();
    let split_views = state.get_all_split_views();
    let layout_trees = state.get_all_layout_trees();
    let window_state = state.window_state.read().clone();

    let layout = Layout {
        workspaces,
        terminals,
        active_workspace_id,
        split_views,
        layout_trees,
        window_state,
    };

    let json_value = serde_json::to_value(&layout)
        .map_err(|e| format!("Failed to serialize layout: {}", e))?;

    store.set(LAYOUT_KEY, json_value);
    store.save().map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}
