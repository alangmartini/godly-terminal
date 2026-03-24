use serde_json::{json, Value};

use godly_protocol::{McpRequest, McpResponse};

use crate::backend::Backend;

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
                "name": "get_active_terminal",
                "description": "Get the currently focused terminal",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "read_terminal",
                "description": "Read raw output from a terminal's rolling 1MB buffer.\n\nBest practices:\n- For running a command and reading its output, prefer `execute_command` (single tool call).\n- Use `read_terminal` when you need historical output, not just the result of the last command.\n- Default mode is 'tail' (last 100 lines). Use strip_ansi=true for clean text.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to read from"
                        },
                        "mode": {
                            "type": "string",
                            "enum": ["full", "head", "tail"],
                            "default": "tail",
                            "description": "Output mode: 'full' returns entire buffer, 'head' returns first N lines, 'tail' (default) returns last N lines"
                        },
                        "lines": {
                            "type": "number",
                            "default": 100,
                            "description": "Number of lines to return (default: 100). Ignored when mode is 'full'."
                        },
                        "filename": {
                            "type": "string",
                            "description": "Save output to file instead of returning it in the response."
                        },
                        "strip_ansi": {
                            "type": "boolean",
                            "description": "Strip ANSI escape codes from the output for clean plain-text. Default: false."
                        }
                    },
                    "required": ["terminal_id"]
                }
            },
            {
                "name": "read_grid",
                "description": "Read the current visible terminal screen as parsed plain text with cursor position.\n\nBest practices:\n- Use this to check what the user sees right now (e.g., prompts, TUI apps, interactive programs).\n- For command output, prefer `execute_command` or `read_terminal` — they capture full output, not just the visible screen.\n- Returns clean text without ANSI escapes, plus cursor coordinates and screen dimensions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to read"
                        }
                    },
                    "required": ["terminal_id"]
                }
            },
            {
                "name": "export_terminal_info",
                "description": "Get a terminal's metadata and example MCP tool calls for cross-session discovery. Useful when one Claude Code session needs to read another terminal's output.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to export info for (optional — defaults to active terminal)"
                        }
                    },
                    "required": []
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
                "name": "get_active_workspace",
                "description": "Get the currently active workspace",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_workspace_details",
                "description": "Get detailed information about a workspace including name, folder path, worktree mode, claude code mode, and terminal count.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to query"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "get_workspace_modes",
                "description": "Get the current worktree_mode and claude_code_mode flags for a workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to query"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "get_split_state",
                "description": "Get the current split-pane configuration for a workspace. Returns the split terminals, direction, and ratio, or null if no split is active.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to query"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "get_layout_tree",
                "description": "Get the full split layout tree for a workspace. Returns a recursive tree structure where each node is either a Leaf (containing a terminal_id) or a Split (containing direction, ratio, and two children). Returns null if the workspace has no split layout.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to query"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "get_tab_order",
                "description": "Get the current tab order (list of terminal IDs) for a workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to query"
                        }
                    },
                    "required": ["workspace_id"]
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
            },
            {
                "name": "get_notification_config",
                "description": "Get the current notification settings: enabled state, sound preset, and volume level.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "list_mute_patterns",
                "description": "List all glob patterns currently used to mute notifications for matching workspaces.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "list_themes",
                "description": "List all available terminal themes and the currently active theme.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_active_theme",
                "description": "Get the name and ID of the currently active terminal theme.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_font_size",
                "description": "Get the current terminal font size in pixels",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_scroll_position",
                "description": "Get the current scroll position of a terminal, including offset, total scrollback lines, and viewport rows. If no terminal_id is provided, uses the active terminal.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to query (optional — defaults to active terminal)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "get_selected_text",
                "description": "Get the currently selected text in the webview. Returns the browser's text selection (if any), or an empty string if nothing is selected.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to get selection from (optional — reads browser selection if omitted)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "list_available_shells",
                "description": "List all supported shell types that can be used as the default shell.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_default_shell",
                "description": "Get the current default shell configuration used for new terminals.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "get_app_info",
                "description": "Get information about the Godly Terminal app: version, workspace count, terminal count, and daemon connection status.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "wait_for_text",
                "description": "Wait for specific text to appear in terminal output.\n\nBest practices:\n- Use this for waiting on specific prompts or markers (e.g., 'Build succeeded', '$ ', 'error:').\n- ANSI codes are stripped before matching. Searches the terminal's rolling 1MB output buffer.\n- Combine with `read_terminal` afterwards if you need the full output context.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to monitor"
                        },
                        "text": {
                            "type": "string",
                            "description": "Text to search for in the terminal output"
                        },
                        "timeout_ms": {
                            "type": "number",
                            "default": 30000,
                            "description": "Maximum time to wait in milliseconds (default: 30000)"
                        }
                    },
                    "required": ["terminal_id", "text"]
                }
            },
            {
                "name": "wait_for_idle",
                "description": "Wait for a terminal to stop producing output (idle detection).\n\nBest practices:\n- For running a command and reading output, prefer `execute_command` (handles wait + read automatically).\n- Use `wait_for_idle` for advanced scenarios: waiting between multiple writes, monitoring long-running processes, or when you need custom idle thresholds.\n- Returns when no output for `idle_ms` milliseconds, or when timeout is reached.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to monitor"
                        },
                        "idle_ms": {
                            "type": "number",
                            "default": 2000,
                            "description": "Milliseconds of silence before considering the terminal idle (default: 2000)"
                        },
                        "timeout_ms": {
                            "type": "number",
                            "default": 30000,
                            "description": "Maximum time to wait in milliseconds (default: 30000)"
                        }
                    },
                    "required": ["terminal_id"]
                }
            },
            {
                "name": "wait_for_app_ready",
                "description": "Wait until the Godly Terminal app is fully initialized and ready for testing.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Maximum time to wait in milliseconds. Default: 30000."
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "capture_screenshot",
                "description": "Capture a screenshot of a terminal's canvas as a PNG file. Returns the file path to the saved screenshot image. If no terminal_id is provided, captures the first visible canvas.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to screenshot (optional — captures first visible canvas if omitted)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "export_state_dump",
                "description": "Export a full dump of the application state (workspaces, terminals, layout trees, active IDs).",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "collect_artifact_bundle",
                "description": "Collect test artifacts (screenshots, state dumps, logs) into a bundle for the given run.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "run_id": {
                            "type": "string",
                            "description": "Test run ID. If omitted, uses the current run or creates a new one."
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "test_harness_status",
                "description": "Get the current status of the staging test harness (ready state, frontend type, run ID, uptime).",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "open_file_pane",
                "description": "Open a file as a viewer pane (code, markdown, or image) split beside a terminal. File type is auto-detected from extension.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "file_path": { "type": "string", "description": "Absolute path to the file to open" },
                        "target_terminal_id": { "type": "string", "description": "Terminal to split beside (optional — defaults to the agent's own terminal)" },
                        "direction": { "type": "string", "enum": ["horizontal", "vertical"], "description": "Split direction (default: horizontal)" },
                        "ratio": { "type": "number", "description": "Split ratio 0.0-1.0, proportion for existing pane (default: 0.5)" }
                    },
                    "required": ["file_path"]
                }
            },
            {
                "name": "close_pane",
                "description": "Close a non-terminal pane (file viewer, markdown preview, or image). Use list_panes to find pane IDs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane_id": { "type": "string", "description": "ID of the pane to close" }
                    },
                    "required": ["pane_id"]
                }
            },
            {
                "name": "list_panes",
                "description": "List all panes in a workspace, including terminals and file viewers. Returns pane IDs, types, and metadata.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": { "type": "string", "description": "Workspace to list panes for (optional — defaults to active workspace)" }
                    },
                    "required": []
                }
            },
            {
                "name": "update_file_pane",
                "description": "Update the file shown in an existing file viewer pane. Reuses the pane without changing the layout.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pane_id": { "type": "string", "description": "ID of the file pane to update" },
                        "file_path": { "type": "string", "description": "New file path to display" }
                    },
                    "required": ["pane_id", "file_path"]
                }
            },
            {
                "name": "ui_query",
                "description": "Query a UI element using a semantic target identifier (e.g. 'workspace.active', 'tab.active', 'terminal.grid').",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Semantic target identifier for the UI element to query."
                        },
                        "args": {
                            "type": "object",
                            "description": "Optional arguments for the query."
                        }
                    },
                    "required": ["target"]
                }
            },
            {
                "name": "ui_wait",
                "description": "Wait for a UI condition to become true (e.g. 'workspace.count >= 2', 'terminal.output.contains').",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "condition": {
                            "type": "string",
                            "description": "Condition expression to wait for."
                        },
                        "timeout_ms": {
                            "type": "integer",
                            "description": "Maximum time to wait in milliseconds. Default: 10000."
                        },
                        "poll_interval_ms": {
                            "type": "integer",
                            "description": "How often to check the condition in milliseconds. Default: 250."
                        },
                        "args": {
                            "type": "object",
                            "description": "Optional arguments for the condition evaluation."
                        }
                    },
                    "required": ["condition"]
                }
            }
        ]
    })
}

/// Dispatch a tool call to the appropriate MCP request.
pub fn call_tool(
    client: &dyn Backend,
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

        "get_active_terminal" => McpRequest::GetActiveTerminal,

        "read_terminal" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            let mode = args.get("mode").and_then(|v| v.as_str()).map(String::from);
            let lines = args.get("lines").and_then(|v| v.as_u64()).map(|n| n as usize);
            let filename = args.get("filename").and_then(|v| v.as_str()).map(String::from);
            let strip_ansi = args.get("strip_ansi").and_then(|v| v.as_bool());

            let request = McpRequest::ReadTerminal {
                terminal_id,
                mode,
                lines,
                strip_ansi,
            };

            let response = client.send_request(&request)?;

            match response {
                McpResponse::TerminalOutput { content } => {
                    if let Some(path) = filename {
                        std::fs::write(&path, &content)
                            .map_err(|e| format!("Failed to write to {}: {}", path, e))?;
                        return Ok(json!({
                            "success": true,
                            "message": format!("Output saved to {}", path),
                            "path": path,
                            "bytes": content.len()
                        }));
                    }
                    return Ok(json!({ "content": content }));
                }
                McpResponse::Error { message } => return Err(message),
                other => return response_to_json(other),
            }
        }

        "read_grid" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            McpRequest::ReadGrid { terminal_id }
        }

        "export_terminal_info" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::ExportTerminalInfo { terminal_id }
        }

        "list_workspaces" => McpRequest::ListWorkspaces,

        "get_active_workspace" => McpRequest::GetActiveWorkspace,

        "get_workspace_details" => {
            let workspace_id = args
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing workspace_id")?
                .to_string();
            McpRequest::GetWorkspaceDetails { workspace_id }
        }

        "get_workspace_modes" => {
            let workspace_id = args
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing workspace_id")?
                .to_string();
            McpRequest::GetWorkspaceModes { workspace_id }
        }

        "get_split_state" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();

            // Send both legacy split state and layout tree requests, merge results
            let split_req = McpRequest::GetSplitState { workspace_id: workspace_id.clone() };
            let tree_req = McpRequest::GetLayoutTree { workspace_id };

            let split_resp = client.send_request(&split_req)?;
            let tree_resp = client.send_request(&tree_req)?;

            let split_json = match split_resp {
                McpResponse::SplitState {
                    workspace_id: _,
                    left_terminal_id,
                    right_terminal_id,
                    direction,
                    ratio,
                } => json!({
                    "left_terminal_id": left_terminal_id,
                    "right_terminal_id": right_terminal_id,
                    "direction": direction,
                    "ratio": ratio,
                }),
                McpResponse::NoSplit => serde_json::Value::Null,
                McpResponse::Error { message } => return Err(message),
                _ => serde_json::Value::Null,
            };

            let tree_json = match tree_resp {
                McpResponse::LayoutTree { tree } => tree
                    .map(|t| serde_json::to_value(t).unwrap_or(serde_json::Value::Null))
                    .unwrap_or(serde_json::Value::Null),
                _ => serde_json::Value::Null,
            };

            return Ok(json!({
                "split": split_json,
                "layout_tree": tree_json,
            }));
        }

        "get_layout_tree" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            McpRequest::GetLayoutTree { workspace_id }
        }

        "get_tab_order" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            McpRequest::GetTabOrder { workspace_id }
        }

        "get_notification_status" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::GetNotificationStatus {
                terminal_id,
                workspace_id,
            }
        }

        "get_notification_config" => McpRequest::GetNotificationConfig,

        "list_mute_patterns" => McpRequest::ListMutePatterns,

        "list_themes" => McpRequest::ListThemes,

        "get_active_theme" => McpRequest::GetActiveTheme,

        "get_font_size" => McpRequest::GetFontSize,

        "get_scroll_position" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::GetScrollPosition { terminal_id }
        }

        "get_selected_text" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::GetSelectedText { terminal_id }
        }

        "list_available_shells" => McpRequest::ListAvailableShells,

        "get_default_shell" => McpRequest::GetDefaultShell,

        "get_app_info" => McpRequest::GetAppInfo,

        "wait_for_text" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            let text = args
                .get("text")
                .and_then(|v| v.as_str())
                .ok_or("Missing text")?
                .to_string();
            let timeout_ms = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(30000);
            McpRequest::WaitForText {
                terminal_id,
                text,
                timeout_ms,
            }
        }

        "wait_for_idle" => {
            let terminal_id = args
                .get("terminal_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing terminal_id")?
                .to_string();
            let idle_ms = args
                .get("idle_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(2000);
            let timeout_ms = args
                .get("timeout_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(30000);
            McpRequest::WaitForIdle {
                terminal_id,
                idle_ms,
                timeout_ms,
            }
        }

        "wait_for_app_ready" => {
            let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64());
            McpRequest::WaitForAppReady { timeout_ms }
        }

        "capture_screenshot" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::CaptureScreenshot { terminal_id }
        }

        "export_state_dump" => McpRequest::ExportStateDump,

        "collect_artifact_bundle" => {
            let run_id = args.get("run_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::CollectArtifactBundle { run_id }
        }

        // Test harness tools
        "test_harness_status" => McpRequest::TestHarnessStatus,

        "ui_query" => {
            let target = args
                .get("target")
                .and_then(|v| v.as_str())
                .ok_or("Missing target")?
                .to_string();
            let query_args = args.get("args").cloned();
            McpRequest::UiQuery { target, args: query_args }
        }

        "ui_wait" => {
            let condition = args
                .get("condition")
                .and_then(|v| v.as_str())
                .ok_or("Missing condition")?
                .to_string();
            let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64());
            let poll_interval_ms = args.get("poll_interval_ms").and_then(|v| v.as_u64());
            let wait_args = args.get("args").cloned();
            McpRequest::UiWait { condition, timeout_ms, poll_interval_ms, args: wait_args }
        }

        "open_file_pane" => {
            let file_path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or("Missing file_path")?
                .to_string();
            let target_terminal_id = args
                .get("target_terminal_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| session_id.clone());
            let direction = args
                .get("direction")
                .and_then(|v| v.as_str())
                .unwrap_or("horizontal")
                .to_string();
            let ratio = args.get("ratio").and_then(|v| v.as_f64()).unwrap_or(0.5);
            McpRequest::OpenFilePane {
                file_path,
                target_terminal_id,
                direction,
                ratio,
            }
        }

        "close_pane" => {
            let pane_id = args
                .get("pane_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing pane_id")?
                .to_string();
            McpRequest::ClosePane { pane_id }
        }

        "list_panes" => {
            let workspace_id = args
                .get("workspace_id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            McpRequest::ListPanes { workspace_id }
        }

        "update_file_pane" => {
            let pane_id = args
                .get("pane_id")
                .and_then(|v| v.as_str())
                .ok_or("Missing pane_id")?
                .to_string();
            let file_path = args
                .get("file_path")
                .and_then(|v| v.as_str())
                .ok_or("Missing file_path")?
                .to_string();
            McpRequest::UpdateFilePane { pane_id, file_path }
        }

        _ => return Err(format!("Unknown tool: {}", name)),
    };

    let response = client.send_request(&request)?;

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
        McpResponse::TerminalOutput { content } => Ok(json!({
            "content": content,
        })),
        McpResponse::WorkspaceDetails {
            name,
            folder_path,
            worktree_mode,
            claude_code_mode,
            terminal_count,
        } => Ok(json!({
            "name": name,
            "folder_path": folder_path,
            "worktree_mode": worktree_mode,
            "claude_code_mode": claude_code_mode,
            "terminal_count": terminal_count,
        })),
        McpResponse::ActiveWorkspace { workspace } => match workspace {
            Some(w) => Ok(json!({
                "id": w.id,
                "name": w.name,
                "folder_path": w.folder_path,
            })),
            None => Ok(json!({ "workspace": null })),
        },
        McpResponse::ActiveTerminal { terminal } => match terminal {
            Some(t) => Ok(json!({
                "id": t.id,
                "workspace_id": t.workspace_id,
                "name": t.name,
                "process_name": t.process_name,
            })),
            None => Ok(json!({ "terminal": null })),
        },
        McpResponse::WaitResult {
            completed,
            last_output_ago_ms,
        } => Ok(json!({
            "completed": completed,
            "last_output_ago_ms": last_output_ago_ms,
        })),
        McpResponse::GridSnapshot {
            rows,
            cursor_row,
            cursor_col,
            cols,
            num_rows,
            alternate_screen,
        } => {
            // Join rows into a single content string, trimming trailing whitespace
            // from each row for a cleaner output.
            let content: String = rows
                .iter()
                .map(|r| r.trim_end())
                .collect::<Vec<_>>()
                .join("\n");
            Ok(json!({
                "content": content,
                "cursor_row": cursor_row,
                "cursor_col": cursor_col,
                "cols": cols,
                "num_rows": num_rows,
                "alternate_screen": alternate_screen,
            }))
        }
        McpResponse::CommandOutput {
            output,
            completed,
            last_output_ago_ms,
            running,
            input_expected,
        } => Ok(json!({
            "output": output,
            "completed": completed,
            "last_output_ago_ms": last_output_ago_ms,
            "running": running,
            "input_expected": input_expected.unwrap_or(false),
        })),
        McpResponse::SplitState {
            workspace_id,
            left_terminal_id,
            right_terminal_id,
            direction,
            ratio,
        } => Ok(json!({
            "workspace_id": workspace_id,
            "left_terminal_id": left_terminal_id,
            "right_terminal_id": right_terminal_id,
            "direction": direction,
            "ratio": ratio,
        })),
        McpResponse::NoSplit => Ok(json!({ "split": null })),
        McpResponse::SplitCreated {
            original_terminal_id,
            new_terminal_id,
            workspace_id,
            direction,
            ratio,
        } => Ok(json!({
            "success": true,
            "original_terminal_id": original_terminal_id,
            "new_terminal_id": new_terminal_id,
            "workspace_id": workspace_id,
            "direction": direction,
            "ratio": ratio,
        })),
        McpResponse::LayoutTree { tree } => Ok(json!({ "layout_tree": tree })),
        McpResponse::JsResult { result, error } => {
            if let Some(err) = error {
                Err(err)
            } else {
                Ok(json!({
                    "result": result.unwrap_or_else(|| "undefined".to_string()),
                }))
            }
        }

        McpResponse::WorkspaceModes {
            worktree_mode,
            claude_code_mode,
        } => Ok(json!({
            "worktree_mode": worktree_mode,
            "claude_code_mode": claude_code_mode,
        })),
        McpResponse::ScrollPosition {
            offset,
            total_scrollback,
            viewport_rows,
        } => Ok(json!({
            "offset": offset,
            "total_scrollback": total_scrollback,
            "viewport_rows": viewport_rows,
        })),
        McpResponse::Screenshot { path } => Ok(json!({
            "path": path,
        })),
        McpResponse::NotificationConfig {
            enabled,
            sound_preset,
            volume,
        } => Ok(json!({
            "enabled": enabled,
            "sound_preset": sound_preset,
            "volume": volume,
        })),
        McpResponse::MutePatterns { patterns } => Ok(json!({
            "patterns": patterns,
        })),
        McpResponse::AppInfo {
            version,
            workspace_count,
            terminal_count,
            daemon_connected,
        } => Ok(json!({
            "version": version,
            "workspace_count": workspace_count,
            "terminal_count": terminal_count,
            "daemon_connected": daemon_connected,
        })),
        McpResponse::TabOrder { terminal_ids } => Ok(json!({
            "terminal_ids": terminal_ids,
        })),
        McpResponse::SelectedText { text } => Ok(json!({
            "text": text,
        })),
        McpResponse::ThemeList { themes, active } => Ok(json!({
            "themes": themes,
            "active": active,
        })),
        McpResponse::AvailableShells { shells } => Ok(json!({
            "shells": shells,
        })),
        McpResponse::ShellInfo {
            shell_type,
            wsl_distribution,
            custom_program,
            custom_args,
        } => {
            let mut obj = json!({ "shell_type": shell_type });
            if let Some(dist) = wsl_distribution {
                obj["wsl_distribution"] = json!(dist);
            }
            if let Some(prog) = custom_program {
                obj["custom_program"] = json!(prog);
            }
            if let Some(args) = custom_args {
                obj["custom_args"] = json!(args);
            }
            Ok(obj)
        }
        McpResponse::FontSize { size } => Ok(json!({
            "font_size": size,
        })),

        // File pane responses
        McpResponse::PaneCreated { pane_id, file_type } => Ok(json!({
            "pane_id": pane_id,
            "file_type": file_type,
        })),
        McpResponse::PaneList { panes } => Ok(json!({
            "panes": panes.iter().map(|p| json!({
                "id": p.id,
                "pane_type": p.pane_type,
                "terminal_id": p.terminal_id,
                "file_path": p.file_path,
                "file_type": p.file_type,
            })).collect::<Vec<_>>(),
        })),

        // Test harness responses
        McpResponse::TestHarnessStatus {
            ready,
            frontend_type,
            harness_mode,
            run_id,
            uptime_ms,
        } => Ok(json!({
            "ready": ready,
            "frontend_type": frontend_type,
            "harness_mode": harness_mode,
            "run_id": run_id,
            "uptime_ms": uptime_ms,
        })),
        McpResponse::StateDump { dump } => Ok(json!({
            "state": dump,
        })),
        McpResponse::ArtifactBundle {
            run_id,
            artifact_dir,
            manifest,
        } => Ok(json!({
            "run_id": run_id,
            "artifact_dir": artifact_dir,
            "manifest": manifest,
        })),
        McpResponse::QueryResult {
            ok,
            target,
            data,
            error,
            timestamp_ms,
        } => Ok(json!({
            "ok": ok,
            "target": target,
            "data": data,
            "error": error,
            "timestamp_ms": timestamp_ms,
        })),
        McpResponse::ActionResult {
            ok,
            target,
            action,
            error,
            timestamp_ms,
        } => Ok(json!({
            "ok": ok,
            "target": target,
            "action": action,
            "error": error,
            "timestamp_ms": timestamp_ms,
        })),
        McpResponse::WaitCompleted {
            ok,
            condition,
            timed_out,
            elapsed_ms,
            error,
        } => Ok(json!({
            "ok": ok,
            "condition": condition,
            "timed_out": timed_out,
            "elapsed_ms": elapsed_ms,
            "error": error,
        })),
    }
}

