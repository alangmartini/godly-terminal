use std::sync::Arc;

use godly_protocol::testing::StateDump;

use crate::state::AppState;

/// Dump the full application state as a `StateDump` for test assertions.
///
/// Serializes workspaces, terminals, and layout trees from `AppState`.
/// Daemon sessions are not yet queried — returns `null` for that field.
pub fn dump_app_state(app_state: &Arc<AppState>) -> StateDump {
    let workspaces = {
        let ws = app_state.get_all_workspaces();
        serde_json::to_value(
            ws.iter()
                .map(|w| {
                    serde_json::json!({
                        "id": w.id,
                        "name": w.name,
                        "folder_path": w.folder_path,
                        "tab_order": w.tab_order,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default()
    };

    let terminals = {
        let terms = app_state.terminals.read();
        serde_json::to_value(
            terms
                .values()
                .map(|t| {
                    serde_json::json!({
                        "id": t.id,
                        "workspace_id": t.workspace_id,
                        "name": t.name,
                        "process_name": t.process_name,
                    })
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default()
    };

    let layout_trees = {
        let trees = app_state.get_all_layout_trees();
        serde_json::to_value(trees).unwrap_or_default()
    };

    let active_workspace_id = app_state.active_workspace_id.read().clone();
    let active_terminal_id = app_state.get_active_terminal_id();

    StateDump {
        workspaces,
        terminals,
        layout_trees,
        daemon_sessions: serde_json::json!(null),
        active_workspace_id,
        active_terminal_id,
    }
}
