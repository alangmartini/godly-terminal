use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use godly_protocol::{Request, Response};

use crate::daemon_client::async_request;
use crate::AppState;

#[derive(Serialize)]
pub struct WorkspaceItem {
    pub id: String,
    pub name: String,
    pub folder_path: String,
    pub shell_type: String,
    pub claude_code_mode: bool,
    pub terminals: Vec<WorkspaceTerminal>,
}

#[derive(Serialize)]
pub struct WorkspaceTerminal {
    pub id: String,
    pub name: String,
    pub shell_type: String,
    pub cwd: Option<String>,
    pub alive: bool,
}

#[derive(Serialize)]
pub struct WorkspacesResponse {
    pub workspaces: Vec<WorkspaceItem>,
    pub active_workspace_id: Option<String>,
}

/// GET /api/workspaces
/// Returns workspace list with terminals, cross-referenced with daemon sessions for alive status.
pub async fn list_workspaces(
    State(state): State<AppState>,
) -> Result<Json<WorkspacesResponse>, (StatusCode, String)> {
    let layout = state.layout_reader.read();

    // Get live session IDs from daemon
    let live_session_ids: Vec<String> = match async_request(&state.daemon, Request::ListSessions).await {
        Ok(Response::SessionList { sessions }) => {
            sessions.into_iter().map(|s| s.id).collect()
        }
        _ => Vec::new(),
    };

    let workspaces = layout
        .workspaces
        .iter()
        .map(|ws| {
            let terminals: Vec<WorkspaceTerminal> = layout
                .terminals
                .iter()
                .filter(|t| t.workspace_id == ws.id)
                .map(|t| WorkspaceTerminal {
                    id: t.id.clone(),
                    name: t.name.clone(),
                    shell_type: t.shell_type.display_name(),
                    cwd: t.cwd.clone(),
                    alive: live_session_ids.contains(&t.id),
                })
                .collect();

            WorkspaceItem {
                id: ws.id.clone(),
                name: ws.name.clone(),
                folder_path: ws.folder_path.clone(),
                shell_type: ws.shell_type.display_name(),
                claude_code_mode: ws.claude_code_mode,
                terminals,
            }
        })
        .collect();

    Ok(Json(WorkspacesResponse {
        workspaces,
        active_workspace_id: layout.active_workspace_id,
    }))
}
