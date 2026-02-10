use serde_json::{json, Value};

use godly_protocol::{McpRequest, McpResponse};

use crate::pipe_client::McpPipeClient;

/// Return the list of MCP tool definitions.
pub fn list_tools() -> Value {
    json!({
        "tools": [
            {
                "name": "get_current_terminal",
                "description": "Get info about the terminal Claude is running in (uses GODLY_SESSION_ID env var)",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "list_terminals",
                "description": "List all terminals with IDs, names, workspace, and process name",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "create_terminal",
                "description": "Create a new terminal in a workspace",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to create the terminal in"
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory for the new terminal (optional)"
                        },
                        "worktree_name": {
                            "type": "string",
                            "description": "Create a git worktree with this name and use it as the terminal's working directory. The workspace must be a git repo. Mutually exclusive with cwd."
                        },
                        "worktree": {
                            "type": "boolean",
                            "description": "Create a git worktree with an auto-generated name. The workspace must be a git repo. Mutually exclusive with cwd. Can be combined with worktree_name for a custom name."
                        },
                        "command": {
                            "type": "string",
                            "description": "A command to run in the terminal immediately after creation. A newline (Enter) is appended automatically."
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "close_terminal",
                "description": "Close a terminal by its ID",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to close"
                        }
                    },
                    "required": ["terminal_id"]
                }
            },
            {
                "name": "rename_terminal",
                "description": "Rename a terminal tab",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to rename"
                        },
                        "name": {
                            "type": "string",
                            "description": "New name for the terminal tab"
                        }
                    },
                    "required": ["terminal_id", "name"]
                }
            },
            {
                "name": "focus_terminal",
                "description": "Switch the active tab to a specific terminal",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to focus"
                        }
                    },
                    "required": ["terminal_id"]
                }
            },
            {
                "name": "list_workspaces",
                "description": "List all workspaces with IDs, names, and folder paths",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "create_workspace",
                "description": "Create a new workspace",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Name for the new workspace"
                        },
                        "folder_path": {
                            "type": "string",
                            "description": "Folder path for the workspace"
                        }
                    },
                    "required": ["name", "folder_path"]
                }
            },
            {
                "name": "switch_workspace",
                "description": "Switch the active workspace",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to switch to"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "move_terminal_to_workspace",
                "description": "Move a terminal to a different workspace",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to move"
                        },
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the target workspace"
                        }
                    },
                    "required": ["terminal_id", "workspace_id"]
                }
            },
            {
                "name": "write_to_terminal",
                "description": "Send text input to another terminal",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to write to"
                        },
                        "data": {
                            "type": "string",
                            "description": "Text to send to the terminal"
                        }
                    },
                    "required": ["terminal_id", "data"]
                }
            },
            {
                "name": "notify",
                "description": "Send a sound notification to alert the user. Plays a chime and shows a badge on the terminal tab if the user isn't looking at it.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "message": {
                            "type": "string",
                            "description": "Optional message to include with the notification"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "set_notification_enabled",
                "description": "Enable or disable sound notifications for a specific terminal or workspace",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to configure (optional)"
                        },
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to configure (optional)"
                        },
                        "enabled": {
                            "type": "boolean",
                            "description": "Whether notifications should be enabled"
                        }
                    },
                    "required": ["enabled"]
                }
            },
            {
                "name": "get_notification_status",
                "description": "Check whether notifications are currently enabled for a terminal or workspace",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to check (optional)"
                        },
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to check (optional)"
                        }
                    },
                    "required": []
                }
            }
        ]
    })
}

/// Dispatch a tool call to the appropriate MCP request.
pub fn call_tool(
    client: &mut McpPipeClient,
    name: &str,
    args: &Value,
    session_id: &Option<String>,
) -> Result<Value, String> {
    let request = match name {
        "get_current_terminal" => {
            let sid = session_id
                .as_ref()
                .ok_or("GODLY_SESSION_ID not set. Is this running inside Godly Terminal?")?;
            McpRequest::GetCurrentSession {
                session_id: sid.clone(),
            }
        }

        "list_terminals" => McpRequest::ListTerminals,

        "create_terminal" => {
            let workspace_id = args
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing workspace_id")?
                .to_string();
            let cwd = args.get("cwd").and_then(|v| v.as_str()).map(String::from);
            let worktree_name = args
                .get("worktree_name")
                .and_then(|v| v.as_str())
                .map(String::from);
            let worktree = args.get("worktree").and_then(|v| v.as_bool());
            let command = args.get("command").and_then(|v| v.as_str()).map(String::from);
            McpRequest::CreateTerminal {
                workspace_id,
                shell_type: None,
                cwd,
                worktree_name,
                worktree,
                command,
            }
        }

        "close_terminal" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            McpRequest::CloseTerminal { terminal_id }
        }

        "rename_terminal" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing name")?
                .to_string();
            McpRequest::RenameTerminal { terminal_id, name }
        }

        "focus_terminal" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            McpRequest::FocusTerminal { terminal_id }
        }

        "list_workspaces" => McpRequest::ListWorkspaces,

        "create_workspace" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or("Missing name")?
                .to_string();
            let folder_path = args
                .get("folder_path")
                .and_then(|v| v.as_str())
                .ok_or("Missing folder_path")?
                .to_string();
            McpRequest::CreateWorkspace { name, folder_path }
        }

        "switch_workspace" => {
            let workspace_id = args
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing workspace_id")?
                .to_string();
            McpRequest::SwitchWorkspace { workspace_id }
        }

        "move_terminal_to_workspace" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            let workspace_id = args
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing workspace_id")?
                .to_string();
            McpRequest::MoveTerminalToWorkspace {
                terminal_id,
                workspace_id,
            }
        }

        "write_to_terminal" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            let data = args
                .get("data")
                .and_then(|v| v.as_str())
                .ok_or("Missing data")?
                .to_string();
            McpRequest::WriteToTerminal { terminal_id, data }
        }

        "notify" => {
            let sid = session_id
                .as_ref()
                .ok_or("GODLY_SESSION_ID not set. Is this running inside Godly Terminal?")?;
            let message = args.get("message").and_then(|v| v.as_str()).map(String::from);
            McpRequest::Notify {
                terminal_id: sid.clone(),
                message,
            }
        }

        "set_notification_enabled" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            let enabled = args
                .get("enabled")
                .and_then(|v| v.as_bool())
                .ok_or("Missing enabled")?;
            McpRequest::SetNotificationEnabled {
                terminal_id,
                workspace_id,
                enabled,
            }
        }

        "get_notification_status" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::GetNotificationStatus {
                terminal_id,
                workspace_id,
            }
        }

        _ => return Err(format!("Unknown tool: {}", name)),
    };

    let response = client
        .send_request(&request)
        .map_err(|e| format!("Pipe error: {}", e))?;

    response_to_json(response)
}

/// Convert an McpResponse to a JSON value suitable for MCP tool call result.
fn response_to_json(response: McpResponse) -> Result<Value, String> {
    match response {
        McpResponse::Ok => Ok(json!({ "success": true })),
        McpResponse::Pong => Ok(json!({ "success": true, "message": "pong" })),
        McpResponse::Error { message } => Err(message),
        McpResponse::TerminalList { terminals } => Ok(json!({
            "terminals": terminals.iter().map(|t| json!({
                "id": t.id,
                "workspace_id": t.workspace_id,
                "name": t.name,
                "process_name": t.process_name,
            })).collect::<Vec<_>>()
        })),
        McpResponse::TerminalInfo { terminal } => Ok(json!({
            "id": terminal.id,
            "workspace_id": terminal.workspace_id,
            "name": terminal.name,
            "process_name": terminal.process_name,
        })),
        McpResponse::WorkspaceList { workspaces } => Ok(json!({
            "workspaces": workspaces.iter().map(|w| json!({
                "id": w.id,
                "name": w.name,
                "folder_path": w.folder_path,
            })).collect::<Vec<_>>()
        })),
        McpResponse::Created {
            id,
            worktree_path,
            worktree_branch,
        } => {
            let mut obj = json!({ "success": true, "id": id });
            if let Some(path) = worktree_path {
                obj["worktree_path"] = json!(path);
            }
            if let Some(branch) = worktree_branch {
                obj["worktree_branch"] = json!(branch);
            }
            Ok(obj)
        }
        McpResponse::NotificationStatus { enabled, source } => Ok(json!({
            "enabled": enabled,
            "source": source,
        })),
    }
}
