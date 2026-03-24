use serde_json::{json, Value};

use godly_protocol::{McpRequest, McpResponse};

use crate::backend::Backend;

/// Return the list of MCP tool definitions.
pub fn list_tools() -> Value {
    json!({
        "tools": [
            // ──────────────────────────────────────────────
            // READ / QUERY TOOLS (existing)
            // ──────────────────────────────────────────────
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
            },

            // ──────────────────────────────────────────────
            // ACTION / MUTATION TOOLS (new)
            // ──────────────────────────────────────────────

            // --- Terminal Management ---
            {
                "name": "create_terminal",
                "description": "Create a new terminal in a workspace. Returns the new terminal ID. Optionally specify shell type, working directory, or a command to run on startup.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to create the terminal in"
                        },
                        "shell_type": {
                            "type": "string",
                            "description": "Shell type (e.g. 'powershell', 'cmd', 'wsl', 'bash'). Uses default shell if omitted."
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory for the new terminal"
                        },
                        "worktree_name": {
                            "type": "string",
                            "description": "Name for a new git worktree to create"
                        },
                        "worktree": {
                            "type": "boolean",
                            "description": "If true, create a git worktree for this terminal"
                        },
                        "command": {
                            "type": "string",
                            "description": "Command to execute on startup (e.g. 'npm run dev')"
                        },
                        "focus": {
                            "type": "boolean",
                            "description": "Whether to focus the new terminal (default: true)"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "close_terminal",
                "description": "Close a terminal and kill its process.",
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
                "description": "Rename a terminal's tab label.",
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
                "description": "Focus/select a terminal, making it the active tab.",
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

            // --- Terminal I/O ---
            {
                "name": "write_to_terminal",
                "description": "Write raw text or keypresses to a terminal. Use this for interactive input — the text is sent exactly as-is to the terminal's PTY. For running a command and reading output, prefer `execute_command`.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to write to"
                        },
                        "data": {
                            "type": "string",
                            "description": "Text to write (sent raw to PTY — include \\r\\n for Enter)"
                        },
                        "focus": {
                            "type": "boolean",
                            "description": "Whether to focus the terminal before writing"
                        }
                    },
                    "required": ["terminal_id", "data"]
                }
            },
            {
                "name": "send_keys",
                "description": "Send named key presses to a terminal. Supports special keys like 'Enter', 'Tab', 'Escape', 'Backspace', 'Up', 'Down', 'Left', 'Right', 'Ctrl+C', 'Ctrl+D', etc.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to send keys to"
                        },
                        "keys": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Array of key names to send (e.g. ['Ctrl+C', 'Enter'])"
                        },
                        "focus": {
                            "type": "boolean",
                            "description": "Whether to focus the terminal before sending keys"
                        }
                    },
                    "required": ["terminal_id", "keys"]
                }
            },
            {
                "name": "erase_content",
                "description": "Erase characters at the cursor (sends Backspace key presses). Useful for clearing typed input.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal"
                        },
                        "count": {
                            "type": "integer",
                            "default": 1,
                            "description": "Number of characters to erase (default: 1)"
                        },
                        "focus": {
                            "type": "boolean",
                            "description": "Whether to focus the terminal first"
                        }
                    },
                    "required": ["terminal_id"]
                }
            },
            {
                "name": "execute_command",
                "description": "Run a shell command in a terminal and wait for it to complete. Returns the command output, completion status, and timing info. This is the recommended way to run commands — it handles write + wait + read in a single call.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to run the command in"
                        },
                        "command": {
                            "type": "string",
                            "description": "Shell command to execute"
                        },
                        "idle_ms": {
                            "type": "number",
                            "default": 2000,
                            "description": "Milliseconds of silence before considering the command done (default: 2000)"
                        },
                        "timeout_ms": {
                            "type": "number",
                            "default": 30000,
                            "description": "Maximum time to wait in milliseconds (default: 30000)"
                        },
                        "focus": {
                            "type": "boolean",
                            "description": "Whether to focus the terminal before running"
                        }
                    },
                    "required": ["terminal_id", "command"]
                }
            },
            {
                "name": "resize_terminal",
                "description": "Resize a terminal to specific dimensions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to resize"
                        },
                        "rows": {
                            "type": "integer",
                            "description": "New row count"
                        },
                        "cols": {
                            "type": "integer",
                            "description": "New column count"
                        }
                    },
                    "required": ["terminal_id", "rows", "cols"]
                }
            },

            // --- Workspace Management ---
            {
                "name": "create_workspace",
                "description": "Create a new workspace with the given name and folder path.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Display name for the workspace"
                        },
                        "folder_path": {
                            "type": "string",
                            "description": "Folder path associated with the workspace"
                        }
                    },
                    "required": ["name", "folder_path"]
                }
            },
            {
                "name": "delete_workspace",
                "description": "Delete a workspace and close all its terminals.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to delete"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "switch_workspace",
                "description": "Switch to a different workspace, making it the active workspace.",
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
                "name": "rename_workspace",
                "description": "Rename a workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to rename"
                        },
                        "name": {
                            "type": "string",
                            "description": "New name for the workspace"
                        }
                    },
                    "required": ["workspace_id", "name"]
                }
            },
            {
                "name": "reorder_workspaces",
                "description": "Reorder the workspace list by providing the desired order of workspace IDs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Ordered list of all workspace IDs"
                        }
                    },
                    "required": ["workspace_ids"]
                }
            },
            {
                "name": "move_terminal_to_workspace",
                "description": "Move a terminal from its current workspace to a different workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to move"
                        },
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the destination workspace"
                        }
                    },
                    "required": ["terminal_id", "workspace_id"]
                }
            },

            // --- Workspace Modes ---
            {
                "name": "toggle_worktree_mode",
                "description": "Toggle git worktree isolation mode for a workspace. When enabled, each new terminal gets its own git worktree.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "toggle_claude_code_mode",
                "description": "Toggle Claude Code mode for a workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "remove_worktree",
                "description": "Remove a git worktree by its path.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "worktree_path": {
                            "type": "string",
                            "description": "Filesystem path of the worktree to remove"
                        }
                    },
                    "required": ["worktree_path"]
                }
            },

            // --- Layout / Split Management ---
            {
                "name": "split_terminal",
                "description": "Split an existing terminal pane into two. Creates a split at the target terminal's position in the layout tree.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace"
                        },
                        "target_terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to split"
                        },
                        "new_terminal_id": {
                            "type": "string",
                            "description": "ID of the new terminal to place in the split"
                        },
                        "direction": {
                            "type": "string",
                            "enum": ["horizontal", "vertical"],
                            "default": "horizontal",
                            "description": "Split direction (default: horizontal)"
                        },
                        "ratio": {
                            "type": "number",
                            "default": 0.5,
                            "description": "Split ratio from 0.0 to 1.0 (default: 0.5)"
                        }
                    },
                    "required": ["workspace_id", "target_terminal_id", "new_terminal_id"]
                }
            },
            {
                "name": "self_split",
                "description": "Split the current terminal session into two panes. Creates a new terminal session alongside the current one.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "session_id": {
                            "type": "string",
                            "description": "Session ID of the terminal to split (uses GODLY_SESSION_ID if omitted)"
                        },
                        "direction": {
                            "type": "string",
                            "enum": ["horizontal", "vertical"],
                            "default": "horizontal",
                            "description": "Split direction (default: horizontal)"
                        },
                        "ratio": {
                            "type": "number",
                            "default": 0.5,
                            "description": "Split ratio (default: 0.5)"
                        },
                        "cwd": {
                            "type": "string",
                            "description": "Working directory for the new pane"
                        },
                        "command": {
                            "type": "string",
                            "description": "Command to run in the new pane"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "unsplit_terminal",
                "description": "Remove a terminal from its split, closing the split and keeping the other pane.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace"
                        },
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to remove from the split"
                        }
                    },
                    "required": ["workspace_id", "terminal_id"]
                }
            },
            {
                "name": "swap_panes",
                "description": "Swap the positions of two terminal panes in the layout.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace"
                        },
                        "terminal_id_a": {
                            "type": "string",
                            "description": "ID of the first terminal"
                        },
                        "terminal_id_b": {
                            "type": "string",
                            "description": "ID of the second terminal"
                        }
                    },
                    "required": ["workspace_id", "terminal_id_a", "terminal_id_b"]
                }
            },
            {
                "name": "zoom_pane",
                "description": "Toggle zoom (maximize/restore) on a terminal pane. When zoomed, the pane fills the entire workspace area.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace"
                        },
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal to zoom (optional — uses active terminal)"
                        }
                    },
                    "required": ["workspace_id"]
                }
            },
            {
                "name": "focus_pane",
                "description": "Move focus to a pane in the given direction relative to the currently focused pane.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional — uses active workspace)"
                        },
                        "direction": {
                            "type": "string",
                            "enum": ["left", "right", "up", "down"],
                            "description": "Direction to move focus"
                        }
                    },
                    "required": ["direction"]
                }
            },
            {
                "name": "focus_other_pane",
                "description": "Move focus to the other pane in a two-pane split.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional — uses active workspace)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "resize_pane",
                "description": "Resize the current split by moving the divider in the given direction.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional — uses active workspace)"
                        },
                        "direction": {
                            "type": "string",
                            "enum": ["left", "right", "up", "down"],
                            "description": "Direction to resize (moves the split divider)"
                        },
                        "delta": {
                            "type": "number",
                            "default": 0.05,
                            "description": "Amount to resize as a fraction of total space (default: 0.05)"
                        }
                    },
                    "required": ["direction"]
                }
            },
            {
                "name": "set_split_ratio",
                "description": "Set the split ratio of the current split to an exact value.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional — uses active workspace)"
                        },
                        "ratio": {
                            "type": "number",
                            "description": "Split ratio from 0.0 to 1.0"
                        }
                    },
                    "required": ["ratio"]
                }
            },
            {
                "name": "rotate_split",
                "description": "Rotate the current split direction (horizontal ↔ vertical).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional — uses active workspace)"
                        }
                    },
                    "required": []
                }
            },

            // --- Tab Navigation ---
            {
                "name": "next_tab",
                "description": "Switch to the next terminal tab in the current workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional — uses active workspace)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "previous_tab",
                "description": "Switch to the previous terminal tab in the current workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional — uses active workspace)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "go_to_tab",
                "description": "Switch to a specific terminal tab by its index (0-based).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional — uses active workspace)"
                        },
                        "index": {
                            "type": "integer",
                            "description": "Tab index (0-based)"
                        }
                    },
                    "required": ["index"]
                }
            },
            {
                "name": "reorder_tabs",
                "description": "Reorder terminal tabs by providing the desired order of terminal IDs.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace"
                        },
                        "terminal_ids": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Ordered list of terminal IDs"
                        }
                    },
                    "required": ["workspace_id", "terminal_ids"]
                }
            },

            // --- Scrollback ---
            {
                "name": "scroll_page_up",
                "description": "Scroll the terminal viewport up by one page.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal (optional — uses active terminal)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "scroll_page_down",
                "description": "Scroll the terminal viewport down by one page.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal (optional — uses active terminal)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "scroll_to_top",
                "description": "Scroll the terminal viewport to the top of the scrollback buffer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal (optional — uses active terminal)"
                        }
                    },
                    "required": []
                }
            },
            {
                "name": "scroll_to_bottom",
                "description": "Scroll the terminal viewport to the bottom (live view).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal (optional — uses active terminal)"
                        }
                    },
                    "required": []
                }
            },

            // --- Appearance ---
            {
                "name": "set_theme",
                "description": "Set the active terminal color theme. Use `list_themes` to see available themes.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "theme_name": {
                            "type": "string",
                            "description": "Name of the theme to activate"
                        }
                    },
                    "required": ["theme_name"]
                }
            },
            {
                "name": "zoom_in",
                "description": "Increase the terminal font size by one step.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "zoom_out",
                "description": "Decrease the terminal font size by one step.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "zoom_reset",
                "description": "Reset the terminal font size to the default.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },

            // --- Shell Settings ---
            {
                "name": "set_default_shell",
                "description": "Set the default shell used for new terminals.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "shell_type": {
                            "type": "string",
                            "description": "Shell type (e.g. 'powershell', 'cmd', 'wsl', 'bash', 'custom')"
                        },
                        "wsl_distribution": {
                            "type": "string",
                            "description": "WSL distribution name (required when shell_type is 'wsl')"
                        },
                        "custom_program": {
                            "type": "string",
                            "description": "Program path (required when shell_type is 'custom')"
                        },
                        "custom_args": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "Arguments for custom program"
                        }
                    },
                    "required": ["shell_type"]
                }
            },

            // --- Notifications ---
            {
                "name": "notify",
                "description": "Trigger a notification for a terminal. The notification appears based on the terminal's notification settings.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal that triggered the notification"
                        },
                        "message": {
                            "type": "string",
                            "description": "Optional notification message"
                        }
                    },
                    "required": ["terminal_id"]
                }
            },
            {
                "name": "set_notification_enabled",
                "description": "Enable or disable notifications for a specific terminal or workspace.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "terminal_id": {
                            "type": "string",
                            "description": "ID of the terminal (optional)"
                        },
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace (optional)"
                        },
                        "enabled": {
                            "type": "boolean",
                            "description": "Whether to enable (true) or disable (false) notifications"
                        }
                    },
                    "required": ["enabled"]
                }
            },
            {
                "name": "set_notification_sound",
                "description": "Set the notification sound preset.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "preset": {
                            "type": "string",
                            "description": "Sound preset name"
                        }
                    },
                    "required": ["preset"]
                }
            },
            {
                "name": "add_mute_pattern",
                "description": "Add a glob pattern to mute notifications for matching workspaces.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern (e.g. '**/node_modules/**')"
                        }
                    },
                    "required": ["pattern"]
                }
            },
            {
                "name": "remove_mute_pattern",
                "description": "Remove a notification mute pattern.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "pattern": {
                            "type": "string",
                            "description": "Glob pattern to remove"
                        }
                    },
                    "required": ["pattern"]
                }
            },

            // --- App Control ---
            {
                "name": "save_layout",
                "description": "Save the current workspace layout to disk. Persists tab order, splits, and workspace configuration.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "required": []
                }
            },
            {
                "name": "open_in_explorer",
                "description": "Open a file or folder in the system file explorer.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "Filesystem path to open"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "copy_to_clipboard",
                "description": "Copy text to the system clipboard.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Text to copy to clipboard"
                        }
                    },
                    "required": ["text"]
                }
            },
            {
                "name": "open_settings",
                "description": "Open the Godly Terminal settings panel.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "tab": {
                            "type": "string",
                            "description": "Settings tab to open (optional)"
                        }
                    },
                    "required": []
                }
            },

            // --- Quick Claude ---
            {
                "name": "quick_claude",
                "description": "Launch a fire-and-forget Claude Code task in a workspace. Creates a new terminal, optionally in a git worktree, and starts Claude Code with the given prompt.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "workspace_id": {
                            "type": "string",
                            "description": "ID of the workspace to launch in"
                        },
                        "prompt": {
                            "type": "string",
                            "description": "Prompt/task for Claude Code to work on"
                        },
                        "branch_name": {
                            "type": "string",
                            "description": "Git branch name for the worktree"
                        },
                        "skip_fetch": {
                            "type": "boolean",
                            "description": "Skip git fetch before creating worktree"
                        },
                        "no_worktree": {
                            "type": "boolean",
                            "description": "Run in the workspace directory instead of a worktree"
                        }
                    },
                    "required": ["workspace_id", "prompt"]
                }
            },

            // --- Semantic Testing ---
            {
                "name": "ui_act",
                "description": "Perform an action on a UI element using a semantic target and action identifier.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "target": {
                            "type": "string",
                            "description": "Semantic target identifier for the UI element."
                        },
                        "action": {
                            "type": "string",
                            "description": "Action to perform on the target."
                        },
                        "args": {
                            "type": "object",
                            "description": "Optional arguments for the action."
                        }
                    },
                    "required": ["target", "action"]
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
        // ──────────────────────────────────────────────
        // READ / QUERY TOOLS (existing)
        // ──────────────────────────────────────────────
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

        // ──────────────────────────────────────────────
        // ACTION / MUTATION TOOLS (new)
        // ──────────────────────────────────────────────

        // --- Terminal Management ---
        "create_terminal" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            // ShellType is a tagged enum — accepts "windows", "pwsh", "cmd",
            // or {"wsl": {"distribution": "..."}} / {"custom": {"program": "...", "args": [...]}}
            let shell_type = args.get("shell_type").and_then(|v| {
                serde_json::from_value::<godly_protocol::types::ShellType>(v.clone()).ok()
            });
            let cwd = args.get("cwd").and_then(|v| v.as_str()).map(String::from);
            let worktree_name = args.get("worktree_name").and_then(|v| v.as_str()).map(String::from);
            let worktree = args.get("worktree").and_then(|v| v.as_bool());
            let command = args.get("command").and_then(|v| v.as_str()).map(String::from);
            let focus = args.get("focus").and_then(|v| v.as_bool());
            McpRequest::CreateTerminal {
                workspace_id,
                shell_type,
                cwd,
                worktree_name,
                worktree,
                command,
                focus,
            }
        }

        "close_terminal" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            McpRequest::CloseTerminal { terminal_id }
        }

        "rename_terminal" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            let name = args.get("name").and_then(|v| v.as_str()).ok_or("Missing name")?.to_string();
            McpRequest::RenameTerminal { terminal_id, name }
        }

        "focus_terminal" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            McpRequest::FocusTerminal { terminal_id }
        }

        // --- Terminal I/O ---
        "write_to_terminal" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            let data = args.get("data").and_then(|v| v.as_str()).ok_or("Missing data")?.to_string();
            let focus = args.get("focus").and_then(|v| v.as_bool());
            McpRequest::WriteToTerminal { terminal_id, data, focus }
        }

        "send_keys" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            let keys: Vec<String> = args.get("keys")
                .and_then(|v| v.as_array())
                .ok_or("Missing keys array")?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            let focus = args.get("focus").and_then(|v| v.as_bool());
            McpRequest::SendKeys { terminal_id, keys, focus }
        }

        "erase_content" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(1) as usize;
            let focus = args.get("focus").and_then(|v| v.as_bool());
            McpRequest::EraseContent { terminal_id, count, focus }
        }

        "execute_command" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            let command = args.get("command").and_then(|v| v.as_str()).ok_or("Missing command")?.to_string();
            let idle_ms = args.get("idle_ms").and_then(|v| v.as_u64()).unwrap_or(2000);
            let timeout_ms = args.get("timeout_ms").and_then(|v| v.as_u64()).unwrap_or(30000);
            let focus = args.get("focus").and_then(|v| v.as_bool());
            McpRequest::ExecuteCommand { terminal_id, command, idle_ms, timeout_ms, focus }
        }

        "resize_terminal" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            let rows = args.get("rows").and_then(|v| v.as_u64()).ok_or("Missing rows")? as u16;
            let cols = args.get("cols").and_then(|v| v.as_u64()).ok_or("Missing cols")? as u16;
            McpRequest::ResizeTerminal { terminal_id, rows, cols }
        }

        // --- Workspace Management ---
        "create_workspace" => {
            let name = args.get("name").and_then(|v| v.as_str()).ok_or("Missing name")?.to_string();
            let folder_path = args.get("folder_path").and_then(|v| v.as_str()).ok_or("Missing folder_path")?.to_string();
            McpRequest::CreateWorkspace { name, folder_path }
        }

        "delete_workspace" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            McpRequest::DeleteWorkspace { workspace_id }
        }

        "switch_workspace" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            McpRequest::SwitchWorkspace { workspace_id }
        }

        "rename_workspace" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            let name = args.get("name").and_then(|v| v.as_str()).ok_or("Missing name")?.to_string();
            McpRequest::RenameWorkspace { workspace_id, name }
        }

        "reorder_workspaces" => {
            let workspace_ids: Vec<String> = args.get("workspace_ids")
                .and_then(|v| v.as_array())
                .ok_or("Missing workspace_ids array")?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            McpRequest::ReorderWorkspaces { workspace_ids }
        }

        "move_terminal_to_workspace" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            McpRequest::MoveTerminalToWorkspace { terminal_id, workspace_id }
        }

        // --- Workspace Modes ---
        "toggle_worktree_mode" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            McpRequest::ToggleWorktreeMode { workspace_id }
        }

        "toggle_claude_code_mode" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            McpRequest::ToggleClaudeCodeMode { workspace_id }
        }

        "remove_worktree" => {
            let worktree_path = args.get("worktree_path").and_then(|v| v.as_str()).ok_or("Missing worktree_path")?.to_string();
            McpRequest::RemoveWorktree { worktree_path }
        }

        // --- Layout / Split Management ---
        "split_terminal" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            let target_terminal_id = args.get("target_terminal_id").and_then(|v| v.as_str()).ok_or("Missing target_terminal_id")?.to_string();
            let new_terminal_id = args.get("new_terminal_id").and_then(|v| v.as_str()).ok_or("Missing new_terminal_id")?.to_string();
            let direction = args.get("direction").and_then(|v| v.as_str()).unwrap_or("horizontal").to_string();
            let ratio = args.get("ratio").and_then(|v| v.as_f64()).unwrap_or(0.5);
            McpRequest::SplitTerminal { workspace_id, target_terminal_id, new_terminal_id, direction, ratio }
        }

        "self_split" => {
            let sid = args.get("session_id").and_then(|v| v.as_str()).map(String::from)
                .or_else(|| session_id.clone())
                .ok_or("Missing session_id (and GODLY_SESSION_ID not set)")?;
            let direction = args.get("direction").and_then(|v| v.as_str()).unwrap_or("horizontal").to_string();
            let ratio = args.get("ratio").and_then(|v| v.as_f64()).unwrap_or(0.5);
            let cwd = args.get("cwd").and_then(|v| v.as_str()).map(String::from);
            let command = args.get("command").and_then(|v| v.as_str()).map(String::from);
            McpRequest::SelfSplit { session_id: sid, direction, ratio, cwd, command }
        }

        "unsplit_terminal" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            McpRequest::UnsplitTerminal { workspace_id, terminal_id }
        }

        "swap_panes" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            let terminal_id_a = args.get("terminal_id_a").and_then(|v| v.as_str()).ok_or("Missing terminal_id_a")?.to_string();
            let terminal_id_b = args.get("terminal_id_b").and_then(|v| v.as_str()).ok_or("Missing terminal_id_b")?.to_string();
            McpRequest::SwapPanes { workspace_id, terminal_id_a, terminal_id_b }
        }

        "zoom_pane" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::ZoomPane { workspace_id, terminal_id }
        }

        "focus_pane" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            let direction = args.get("direction").and_then(|v| v.as_str()).ok_or("Missing direction")?.to_string();
            McpRequest::FocusPane { workspace_id, direction }
        }

        "focus_other_pane" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::FocusOtherPane { workspace_id }
        }

        "resize_pane" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            let direction = args.get("direction").and_then(|v| v.as_str()).ok_or("Missing direction")?.to_string();
            let delta = args.get("delta").and_then(|v| v.as_f64()).unwrap_or(0.05);
            McpRequest::ResizePane { workspace_id, direction, delta }
        }

        "set_split_ratio" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            let ratio = args.get("ratio").and_then(|v| v.as_f64()).ok_or("Missing ratio")?;
            McpRequest::SetSplitRatio { workspace_id, ratio }
        }

        "rotate_split" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::RotateSplit { workspace_id }
        }

        // --- Tab Navigation ---
        "next_tab" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::NextTab { workspace_id }
        }

        "previous_tab" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::PreviousTab { workspace_id }
        }

        "go_to_tab" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            let index = args.get("index").and_then(|v| v.as_u64()).ok_or("Missing index")? as u32;
            McpRequest::GoToTab { workspace_id, index }
        }

        "reorder_tabs" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            let terminal_ids: Vec<String> = args.get("terminal_ids")
                .and_then(|v| v.as_array())
                .ok_or("Missing terminal_ids array")?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            McpRequest::ReorderTabs { workspace_id, terminal_ids }
        }

        // --- Scrollback ---
        "scroll_page_up" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::ScrollPageUp { terminal_id }
        }

        "scroll_page_down" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::ScrollPageDown { terminal_id }
        }

        "scroll_to_top" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::ScrollToTop { terminal_id }
        }

        "scroll_to_bottom" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            McpRequest::ScrollToBottom { terminal_id }
        }

        // --- Appearance ---
        "set_theme" => {
            let theme_name = args.get("theme_name").and_then(|v| v.as_str()).ok_or("Missing theme_name")?.to_string();
            McpRequest::SetTheme { theme_name }
        }

        "zoom_in" => McpRequest::ZoomIn,
        "zoom_out" => McpRequest::ZoomOut,
        "zoom_reset" => McpRequest::ZoomReset,

        // --- Shell Settings ---
        "set_default_shell" => {
            let shell_type = args.get("shell_type").and_then(|v| v.as_str()).ok_or("Missing shell_type")?.to_string();
            let wsl_distribution = args.get("wsl_distribution").and_then(|v| v.as_str()).map(String::from);
            let custom_program = args.get("custom_program").and_then(|v| v.as_str()).map(String::from);
            let custom_args = args.get("custom_args").and_then(|v| v.as_array()).map(|arr| {
                arr.iter().filter_map(|v| v.as_str().map(String::from)).collect()
            });
            McpRequest::SetDefaultShell { shell_type, wsl_distribution, custom_program, custom_args }
        }

        // --- Notifications ---
        "notify" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).ok_or("Missing terminal_id")?.to_string();
            let message = args.get("message").and_then(|v| v.as_str()).map(String::from);
            McpRequest::Notify { terminal_id, message }
        }

        "set_notification_enabled" => {
            let terminal_id = args.get("terminal_id").and_then(|v| v.as_str()).map(String::from);
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).map(String::from);
            let enabled = args.get("enabled").and_then(|v| v.as_bool()).ok_or("Missing enabled")?;
            McpRequest::SetNotificationEnabled { terminal_id, workspace_id, enabled }
        }

        "set_notification_sound" => {
            let preset = args.get("preset").and_then(|v| v.as_str()).ok_or("Missing preset")?.to_string();
            McpRequest::SetNotificationSound { preset }
        }

        "add_mute_pattern" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).ok_or("Missing pattern")?.to_string();
            McpRequest::AddMutePattern { pattern }
        }

        "remove_mute_pattern" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).ok_or("Missing pattern")?.to_string();
            McpRequest::RemoveMutePattern { pattern }
        }

        // --- App Control ---
        "save_layout" => McpRequest::SaveLayout,

        "open_in_explorer" => {
            let path = args.get("path").and_then(|v| v.as_str()).ok_or("Missing path")?.to_string();
            McpRequest::OpenInExplorer { path }
        }

        "copy_to_clipboard" => {
            let text = args.get("text").and_then(|v| v.as_str()).ok_or("Missing text")?.to_string();
            McpRequest::CopyToClipboard { text }
        }

        "open_settings" => {
            let tab = args.get("tab").and_then(|v| v.as_str()).map(String::from);
            McpRequest::OpenSettings { tab }
        }

        // --- Quick Claude ---
        "quick_claude" => {
            let workspace_id = args.get("workspace_id").and_then(|v| v.as_str()).ok_or("Missing workspace_id")?.to_string();
            let prompt = args.get("prompt").and_then(|v| v.as_str()).ok_or("Missing prompt")?.to_string();
            let branch_name = args.get("branch_name").and_then(|v| v.as_str()).map(String::from);
            let skip_fetch = args.get("skip_fetch").and_then(|v| v.as_bool());
            let no_worktree = args.get("no_worktree").and_then(|v| v.as_bool());
            McpRequest::QuickClaude { workspace_id, prompt, branch_name, skip_fetch, no_worktree }
        }

        // --- Semantic Testing ---
        "ui_act" => {
            let target = args.get("target").and_then(|v| v.as_str()).ok_or("Missing target")?.to_string();
            let action = args.get("action").and_then(|v| v.as_str()).ok_or("Missing action")?.to_string();
            let act_args = args.get("args").cloned();
            McpRequest::UiAct { target, action, args: act_args }
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
