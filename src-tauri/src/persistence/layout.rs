use std::sync::Arc;
use tauri::{AppHandle, State};
use tauri_plugin_store::StoreExt;

use crate::state::{AppState, Layout, TerminalInfo};

const STORE_PATH: &str = "layout.json";
const LAYOUT_KEY: &str = "layout";

#[tauri::command]
pub fn save_layout(app_handle: AppHandle, state: State<Arc<AppState>>) -> Result<(), String> {
    let store = app_handle
        .store(STORE_PATH)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    let workspaces = state.get_all_workspaces();
    let terminals: Vec<TerminalInfo> = {
        let terminals = state.terminals.read();
        terminals
            .values()
            .map(|t| TerminalInfo {
                id: t.id.clone(),
                workspace_id: t.workspace_id.clone(),
                name: t.name.clone(),
            })
            .collect()
    };
    let active_workspace_id = state.active_workspace_id.read().clone();

    let layout = Layout {
        workspaces,
        terminals,
        active_workspace_id,
    };

    store.set(LAYOUT_KEY, serde_json::to_value(&layout).unwrap());
    store.save().map_err(|e| format!("Failed to save store: {}", e))?;

    Ok(())
}

#[tauri::command]
pub fn load_layout(app_handle: AppHandle) -> Result<Layout, String> {
    let store = app_handle
        .store(STORE_PATH)
        .map_err(|e| format!("Failed to open store: {}", e))?;

    match store.get(LAYOUT_KEY) {
        Some(value) => serde_json::from_value(value.clone())
            .map_err(|e| format!("Failed to parse layout: {}", e)),
        None => Ok(Layout::default()),
    }
}

pub fn save_on_exit(app_handle: &AppHandle, state: &Arc<AppState>) {
    let store = match app_handle.store(STORE_PATH) {
        Ok(s) => s,
        Err(_) => return,
    };

    let workspaces = state.get_all_workspaces();
    let terminals: Vec<TerminalInfo> = {
        let terminals = state.terminals.read();
        terminals
            .values()
            .map(|t| TerminalInfo {
                id: t.id.clone(),
                workspace_id: t.workspace_id.clone(),
                name: t.name.clone(),
            })
            .collect()
    };
    let active_workspace_id = state.active_workspace_id.read().clone();

    let layout = Layout {
        workspaces,
        terminals,
        active_workspace_id,
    };

    store.set(LAYOUT_KEY, serde_json::to_value(&layout).unwrap());
    let _ = store.save();
}
