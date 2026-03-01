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
    /// Redacted to folder name only (not full path) to prevent information disclosure.
    pub folder_path: String,
    pub shell_type: String,
    pub ai_tool_mode: String,
    pub terminals: Vec<WorkspaceTerminal>,
}

#[derive(Serialize)]
pub struct WorkspaceTerminal {
    pub id: String,
    pub name: String,
    /// OSC window title from the running program (e.g. "claude: fixing bug").
    pub title: String,
    pub shell_type: String,
    /// Redacted to last 2 path components to prevent full filesystem path disclosure.
    pub cwd: Option<String>,
    pub alive: bool,
}

#[derive(Serialize)]
pub struct WorkspacesResponse {
    pub workspaces: Vec<WorkspaceItem>,
    pub active_workspace_id: Option<String>,
}

/// Redact a full path to just the last component (folder name).
/// "C:\Users\alice\Documents\secret-project" → "secret-project"
fn redact_path(full_path: &str) -> String {
    std::path::Path::new(full_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| full_path.to_string())
}

/// Redact a CWD to the last 2 path components.
/// "C:\Users\alice\work\my-project\src" → "my-project/src"
fn redact_cwd(full_path: &str) -> String {
    let path = std::path::Path::new(full_path);
    let components: Vec<_> = path.components().collect();
    if components.len() <= 2 {
        return redact_path(full_path);
    }
    let tail: Vec<_> = components[components.len() - 2..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect();
    tail.join("/")
}

/// GET /api/workspaces
/// Returns workspace list with terminals, cross-referenced with daemon sessions for alive status.
/// Full filesystem paths are redacted to prevent information disclosure over the network.
pub async fn list_workspaces(
    State(state): State<AppState>,
) -> Result<Json<WorkspacesResponse>, (StatusCode, String)> {
    let layout = state.layout_reader.read();

    // Get live sessions from daemon (includes OSC title from VT parser)
    let live_sessions: std::collections::HashMap<String, godly_protocol::SessionInfo> =
        match async_request(&state.daemon, Request::ListSessions).await {
            Ok(Response::SessionList { sessions }) => {
                sessions.into_iter().map(|s| (s.id.clone(), s)).collect()
            }
            _ => std::collections::HashMap::new(),
        };

    let workspaces = layout
        .workspaces
        .iter()
        .map(|ws| {
            let terminals: Vec<WorkspaceTerminal> = layout
                .terminals
                .iter()
                .filter(|t| t.workspace_id == ws.id)
                .map(|t| {
                    let session = live_sessions.get(&t.id);
                    WorkspaceTerminal {
                        id: t.id.clone(),
                        name: t.name.clone(),
                        title: session.map(|s| s.title.clone()).unwrap_or_default(),
                        shell_type: t.shell_type.display_name(),
                        cwd: t.cwd.as_deref().map(redact_cwd),
                        alive: session.is_some(),
                    }
                })
                .collect();

            WorkspaceItem {
                id: ws.id.clone(),
                name: ws.name.clone(),
                folder_path: redact_path(&ws.folder_path),
                shell_type: ws.shell_type.display_name(),
                ai_tool_mode: serde_json::to_string(&ws.ai_tool_mode).unwrap_or_else(|_| "\"none\"".to_string()).trim_matches('"').to_string(),
                terminals,
            }
        })
        .collect();

    Ok(Json(WorkspacesResponse {
        workspaces,
        active_workspace_id: layout.active_workspace_id,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_path_extracts_folder_name() {
        assert_eq!(redact_path(r"C:\Users\alice\Documents\my-project"), "my-project");
        assert_eq!(redact_path("/home/alice/work/repo"), "repo");
        assert_eq!(redact_path("folder"), "folder");
    }

    #[test]
    fn redact_cwd_shows_last_two_components() {
        assert_eq!(redact_cwd(r"C:\Users\alice\work\my-project\src"), "my-project/src");
        assert_eq!(redact_cwd("/home/alice/work"), "alice/work");
        assert_eq!(redact_cwd("folder"), "folder");
    }
}
