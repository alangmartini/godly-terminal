use godly_app_adapter::mcp_pipe::McpEvent;
use godly_app_adapter::sound;
use godly_layout_core::{LayoutNode, SplitDirection};
use godly_protocol::testing::StateDump;
use godly_protocol::{McpRequest, McpResponse, McpTerminalInfo, McpWorkspaceInfo};
use iced::window;
use serde_json::{json, Value};

use crate::app::{GodlyApp, Message};
use crate::session_persistence::PersistedLayoutNode;
use crate::shortcuts_tab;

impl GodlyApp {
    /// Handle an incoming MCP event by mutating app state and returning any
    /// follow-up tasks (e.g., grid fetch, terminal creation).
    pub(crate) fn handle_mcp_event(&mut self, event: McpEvent) -> iced::Task<Message> {
        match event {
            McpEvent::Request { request, reply } => {
                // CaptureScreenshot is async — defer the reply until iced
                // delivers the screenshot via Message::ScreenshotCaptured.
                if matches!(request, McpRequest::CaptureScreenshot { .. }) {
                    let Some(wid) = self.window_id else {
                        let _ = reply.send(McpResponse::Error {
                            message: "Window not yet initialized".to_string(),
                        });
                        return iced::Task::none();
                    };
                    self.pending_screenshot_reply = Some(reply);
                    return window::screenshot(wid).map(Message::ScreenshotCaptured);
                }

                let (response, task) = self.handle_mcp_request(request);
                let _ = reply.send(response);
                task
            }

            // J1: Focus a terminal — switch to its workspace and set as focused.
            McpEvent::FocusTerminal { terminal_id } => {
                if let Some(ws_id) = self.find_workspace_for_terminal(&terminal_id) {
                    self.workspaces.set_active(&ws_id);
                    if let Some(ws) = self.workspaces.get_mut(&ws_id) {
                        ws.focused_terminal = terminal_id.clone();
                    }
                    self.terminals.set_active(&terminal_id);
                }
                iced::Task::none()
            }

            // J2: Switch to a workspace by ID.
            McpEvent::SwitchWorkspace { workspace_id } => {
                self.workspaces.set_active(&workspace_id);
                iced::Task::none()
            }

            // J3: Rename a terminal (set custom_name).
            McpEvent::RenameTerminal { terminal_id, name } => {
                if let Some(term) = self.terminals.get_mut(&terminal_id) {
                    term.custom_name = Some(name);
                }
                iced::Task::none()
            }

            // J4: Create a terminal — delegate to the existing NewTabRequested flow
            // which handles daemon IPC for session creation.
            McpEvent::CreateTerminal { .. } => {
                iced::Task::done(Message::NewTabRequested)
            }

            // J5: Close a terminal.
            McpEvent::CloseTerminal { terminal_id } => {
                iced::Task::done(Message::CloseTabRequested(terminal_id))
            }

            // J6: Move a terminal to a different workspace.
            McpEvent::MoveTerminal {
                terminal_id,
                workspace_id,
            } => {
                // Remove from source workspace layout.
                let source_id = self.find_workspace_for_terminal(&terminal_id);
                if let Some(source_id) = source_id {
                    if source_id != workspace_id {
                        if let Some(source_ws) = self.workspaces.get_mut(&source_id) {
                            source_ws.layout.unsplit_leaf(&terminal_id);
                            if source_ws.focused_terminal == terminal_id {
                                if let Some(first) = source_ws.layout.all_leaf_ids().first() {
                                    source_ws.focused_terminal = first.to_string();
                                }
                            }
                        }
                    }
                }

                // Add to target workspace layout — insert as a split alongside the focused pane.
                if let Some(target_ws) = self.workspaces.get_mut(&workspace_id) {
                    if !target_ws.layout.find_leaf(&terminal_id) {
                        let target_focused = target_ws.focused_terminal.clone();
                        target_ws.layout.split_leaf(
                            &target_focused,
                            terminal_id.clone(),
                            SplitDirection::Horizontal,
                        );
                    }
                    target_ws.focused_terminal = terminal_id.clone();
                }

                // Update the terminal's workspace_id.
                if let Some(term) = self.terminals.get_mut(&terminal_id) {
                    term.workspace_id = Some(workspace_id);
                }

                iced::Task::none()
            }

            // J7: Push a toast notification for a terminal.
            McpEvent::Notify {
                terminal_id,
                message,
            } => {
                let msg = message.unwrap_or_else(|| "Notification".to_string());
                let title = if let Some(term) = self.terminals.get(&terminal_id) {
                    term.tab_label().to_string()
                } else {
                    terminal_id.clone()
                };
                self.enqueue_toast(title, msg);
                self.play_notification_sound_if_allowed(&terminal_id);
                iced::Task::none()
            }

            // J8: Split a terminal pane.
            McpEvent::SplitTerminal {
                workspace_id,
                target_terminal_id,
                new_terminal_id,
                direction,
                ..
            } => {
                let dir = match direction.as_str() {
                    "vertical" => SplitDirection::Vertical,
                    _ => SplitDirection::Horizontal,
                };
                if let Some(ws) = self.workspaces.get_mut(&workspace_id) {
                    ws.layout
                        .split_leaf(&target_terminal_id, new_terminal_id.clone(), dir);
                    ws.focused_terminal = new_terminal_id;
                }
                self.resize_all_terminals()
            }

            // J8: Unsplit — remove a terminal from its split.
            McpEvent::UnsplitTerminal {
                workspace_id,
                terminal_id,
            } => {
                if let Some(ws) = self.workspaces.get_mut(&workspace_id) {
                    ws.layout.unsplit_leaf(&terminal_id);
                    if ws.focused_terminal == terminal_id {
                        if let Some(first) = ws.layout.all_leaf_ids().first() {
                            ws.focused_terminal = first.to_string();
                        }
                    }
                }
                self.resize_all_terminals()
            }

            // J9: Swap two panes in a layout.
            McpEvent::SwapPanes {
                workspace_id,
                terminal_id_a,
                terminal_id_b,
            } => {
                if let Some(ws) = self.workspaces.get_mut(&workspace_id) {
                    swap_leaves_in_layout(&mut ws.layout, &terminal_id_a, &terminal_id_b);
                }
                iced::Task::none()
            }

            // J9: Zoom/unzoom a pane (toggle).
            // Full zoom support requires UI-level maximization; for now, focus the pane.
            McpEvent::ZoomPane {
                workspace_id,
                terminal_id,
            } => {
                if let Some(ws) = self.workspaces.get_mut(&workspace_id) {
                    if let Some(tid) = terminal_id {
                        ws.focused_terminal = tid;
                    }
                }
                iced::Task::none()
            }
        }
    }

    fn handle_mcp_request(&mut self, request: McpRequest) -> (McpResponse, iced::Task<Message>) {
        match request {
            McpRequest::ListTerminals => (
                McpResponse::TerminalList {
                    terminals: self
                        .terminals
                        .iter()
                        .map(|terminal| self.to_mcp_terminal_info(terminal))
                        .collect(),
                },
                iced::Task::none(),
            ),
            McpRequest::GetActiveTerminal => (
                McpResponse::ActiveTerminal {
                    terminal: self
                        .terminals
                        .active()
                        .map(|terminal| self.to_mcp_terminal_info(terminal)),
                },
                iced::Task::none(),
            ),
            McpRequest::CreateTerminal {
                workspace_id,
                cwd,
                ..
            } => match self.create_terminal_for_testing(&workspace_id, cwd, None) {
                Ok((terminal_id, task)) => (
                    McpResponse::Created {
                        id: terminal_id,
                        worktree_path: None,
                        worktree_branch: None,
                    },
                    task,
                ),
                Err(error) => (McpResponse::Error { message: error }, iced::Task::none()),
            },
            McpRequest::ListWorkspaces => (
                McpResponse::WorkspaceList {
                    workspaces: self
                        .workspaces
                        .iter()
                        .map(Self::to_mcp_workspace_info)
                        .collect(),
                },
                iced::Task::none(),
            ),
            McpRequest::GetActiveWorkspace => (
                McpResponse::ActiveWorkspace {
                    workspace: self.workspaces.active().map(Self::to_mcp_workspace_info),
                },
                iced::Task::none(),
            ),
            McpRequest::CreateWorkspace { name, folder_path } => {
                match self.create_workspace_for_testing(name, folder_path) {
                    Ok((workspace_id, task)) => (
                        McpResponse::Created {
                            id: workspace_id,
                            worktree_path: None,
                            worktree_branch: None,
                        },
                        task,
                    ),
                    Err(error) => (McpResponse::Error { message: error }, iced::Task::none()),
                }
            }
            McpRequest::DeleteWorkspace { workspace_id } => {
                if self.workspaces.get(&workspace_id).is_none() {
                    return (
                        McpResponse::Error {
                            message: format!("Workspace {} not found", workspace_id),
                        },
                        iced::Task::none(),
                    );
                }
                if self.workspaces.count() <= 1 {
                    return (
                        McpResponse::Error {
                            message: "Cannot delete the last workspace".to_string(),
                        },
                        iced::Task::none(),
                    );
                }
                if self.last_test_workspace_id.as_deref() == Some(workspace_id.as_str()) {
                    self.last_test_workspace_id = None;
                }
                (McpResponse::Ok, self.delete_workspace(&workspace_id))
            }
            McpRequest::SaveLayout => match self.save_layout_for_testing() {
                Ok(()) => (McpResponse::Ok, iced::Task::none()),
                Err(error) => (McpResponse::Error { message: error }, iced::Task::none()),
            },
            McpRequest::GetAppInfo => (
                McpResponse::AppInfo {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    workspace_count: self.workspaces.count(),
                    terminal_count: self.terminals.count(),
                    daemon_connected: self.client.is_some(),
                },
                iced::Task::none(),
            ),
            McpRequest::TestHarnessStatus | McpRequest::WaitForAppReady { .. } => (
                McpResponse::TestHarnessStatus {
                    ready: self.workspaces.count() > 0 && self.terminals.count() > 0,
                    frontend_type: "native".to_string(),
                    harness_mode: std::env::var("GODLY_TEST_HARNESS")
                        .map(|value| value == "1")
                        .unwrap_or(false),
                    run_id: None,
                    uptime_ms: Self::now_ms().saturating_sub(self.started_at_ms),
                },
                iced::Task::none(),
            ),
            McpRequest::ResetStagingProfile => match self.reset_staging_profile_for_testing() {
                Ok(task) => (McpResponse::Ok, task),
                Err(error) => (McpResponse::Error { message: error }, iced::Task::none()),
            },
            McpRequest::ExportStateDump => (
                McpResponse::StateDump {
                    dump: self.build_state_dump_json(),
                },
                iced::Task::none(),
            ),
            McpRequest::UiQuery { target, args } => (
                self.handle_ui_query(&target, args.as_ref()),
                iced::Task::none(),
            ),
            McpRequest::UiAct {
                target,
                action,
                args,
            } => self.handle_ui_action(&target, &action, args.as_ref()),
            McpRequest::UiWait { condition, args, .. } => (
                self.handle_ui_wait(&condition, args.as_ref()),
                iced::Task::none(),
            ),

            // -- Notification settings --
            McpRequest::GetNotificationConfig => (
                McpResponse::NotificationConfig {
                    enabled: self.notification_sounds_enabled,
                    sound_preset: self.notification_sound_preset.label().to_string(),
                    volume: 1.0,
                },
                iced::Task::none(),
            ),
            McpRequest::SetNotificationSound { preset } => {
                match sound::NotificationSoundPreset::from_label(&preset) {
                    Some(p) => {
                        self.notification_sound_preset = p;
                        (McpResponse::Ok, iced::Task::none())
                    }
                    None => (
                        McpResponse::Error {
                            message: format!(
                                "Unknown preset '{}'. Valid: none, chime, bell, ping, peon",
                                preset
                            ),
                        },
                        iced::Task::none(),
                    ),
                }
            }
            McpRequest::SetNotificationEnabled {
                enabled, ..
            } => {
                self.notification_sounds_enabled = enabled;
                (McpResponse::Ok, iced::Task::none())
            }
            McpRequest::GetNotificationStatus { .. } => (
                McpResponse::NotificationStatus {
                    enabled: self.notification_sounds_enabled,
                    source: "global".to_string(),
                },
                iced::Task::none(),
            ),
            McpRequest::AddMutePattern { pattern } => {
                if !self.workspace_mute_patterns.contains(&pattern) {
                    self.workspace_mute_patterns.push(pattern);
                }
                (McpResponse::Ok, iced::Task::none())
            }
            McpRequest::RemoveMutePattern { pattern } => {
                self.workspace_mute_patterns.retain(|p| p != &pattern);
                (McpResponse::Ok, iced::Task::none())
            }
            McpRequest::ListMutePatterns => (
                McpResponse::MutePatterns {
                    patterns: self.workspace_mute_patterns.clone(),
                },
                iced::Task::none(),
            ),

            McpRequest::ToggleWorktreeMode { workspace_id } => {
                if let Some(ws) = self.workspaces.get(&workspace_id) {
                    let new_mode = !ws.worktree_mode;
                    let _ = self.workspaces.set_worktree_mode(&workspace_id, new_mode);
                    let claude_code_mode = false; // TODO: read actual value
                    (
                        McpResponse::WorkspaceModes {
                            worktree_mode: new_mode,
                            claude_code_mode,
                        },
                        iced::Task::none(),
                    )
                } else {
                    (
                        McpResponse::Error {
                            message: format!("Workspace {} not found", workspace_id),
                        },
                        iced::Task::none(),
                    )
                }
            }

            McpRequest::RemoveWorktree { worktree_path } => {
                // Find repo root from any active workspace.
                let repo_root = self.workspaces.active().map(|ws| ws.folder_path.clone());
                if let Some(root) = repo_root {
                    match crate::git_worktree::remove_worktree(&root, &worktree_path) {
                        Ok(()) => (McpResponse::Ok, iced::Task::none()),
                        Err(e) => (
                            McpResponse::Error {
                                message: format!("Failed to remove worktree: {e}"),
                            },
                            iced::Task::none(),
                        ),
                    }
                } else {
                    (
                        McpResponse::Error {
                            message: "No active workspace to determine repo root".to_string(),
                        },
                        iced::Task::none(),
                    )
                }
            }

            other => (
                McpResponse::Error {
                    message: format!("Unsupported native MCP request: {:?}", other),
                },
                iced::Task::none(),
            ),
        }
    }

    fn handle_ui_query(&self, target: &str, args: Option<&Value>) -> McpResponse {
        match self.resolve_ui_query(target, args) {
            Ok(data) => McpResponse::QueryResult {
                ok: true,
                target: target.to_string(),
                data: Some(data),
                error: None,
                timestamp_ms: Self::now_ms(),
            },
            Err(error) => McpResponse::QueryResult {
                ok: false,
                target: target.to_string(),
                data: None,
                error: Some(error),
                timestamp_ms: Self::now_ms(),
            },
        }
    }

    fn handle_ui_action(
        &mut self,
        target: &str,
        action: &str,
        args: Option<&Value>,
    ) -> (McpResponse, iced::Task<Message>) {
        let key = format!("{}.{}", target, action);
        match key.as_str() {
            "workspace.create" => {
                let name = self
                    .string_arg(args, "name")
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| format!("Workspace {}", self.next_workspace_num));
                let folder_path = self
                    .string_arg(args, "folder_path")
                    .or_else(|| {
                        self.workspaces
                            .active()
                            .map(|workspace| workspace.folder_path.clone())
                    })
                    .unwrap_or_else(default_workspace_folder);
                match self.create_workspace_for_testing(name, folder_path) {
                    Ok((_workspace_id, task)) => (
                        McpResponse::ActionResult {
                            ok: true,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: None,
                            timestamp_ms: Self::now_ms(),
                        },
                        task,
                    ),
                    Err(error) => (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some(error),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    ),
                }
            }
            "workspace.switch" => {
                let Some(workspace_id) = self.resolve_workspace_id_from_args(args, true, false) else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("workspace_id required".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                if self.workspaces.get(&workspace_id).is_none() {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some(format!("Workspace {} not found", workspace_id)),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                }
                self.last_test_workspace_id = Some(workspace_id.clone());
                self.workspaces.set_active(&workspace_id);
                if let Some(workspace) = self.workspaces.get(&workspace_id) {
                    self.terminals.set_active(&workspace.focused_terminal);
                }
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            "workspace.delete" => {
                let Some(workspace_id) = self.resolve_workspace_id_from_args(args, true, false) else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("workspace_id required".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                if self.workspaces.get(&workspace_id).is_none() {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some(format!("Workspace {} not found", workspace_id)),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                }
                if self.workspaces.count() <= 1 {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("Cannot delete the last workspace".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                }
                if self.last_test_workspace_id.as_deref() == Some(workspace_id.as_str()) {
                    self.last_test_workspace_id = None;
                }
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    self.delete_workspace(&workspace_id),
                )
            }
            "terminal.create" => {
                let workspace_id = self
                    .resolve_workspace_id_from_args(args, true, true)
                    .or_else(|| self.workspaces.active_id().map(str::to_string));
                let Some(workspace_id) = workspace_id else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("No active workspace".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                let cwd = self.string_arg(args, "cwd");
                let name = self.string_arg(args, "name");
                match self.create_terminal_for_testing(&workspace_id, cwd, name) {
                    Ok((_terminal_id, task)) => (
                        McpResponse::ActionResult {
                            ok: true,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: None,
                            timestamp_ms: Self::now_ms(),
                        },
                        task,
                    ),
                    Err(error) => (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some(error),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    ),
                }
            }
            "terminal.focus" => {
                let Some(terminal_id) = self.resolve_terminal_id_from_args(args, true) else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("terminal_id required".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                if let Some(workspace_id) = self.find_workspace_for_terminal(&terminal_id) {
                    self.workspaces.set_active(&workspace_id);
                    if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
                        workspace.focused_terminal = terminal_id.clone();
                    }
                }
                self.terminals.set_active(&terminal_id);
                self.last_test_terminal_id = Some(terminal_id);
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            "terminal.close" => {
                let Some(terminal_id) = self.resolve_terminal_id_from_args(args, true) else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("terminal_id required".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                if self.terminals.get(&terminal_id).is_none() {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some(format!("Terminal {} not found", terminal_id)),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                }
                if self.last_test_terminal_id.as_deref() == Some(terminal_id.as_str()) {
                    self.last_test_terminal_id = None;
                }
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    self.close_terminal(&terminal_id),
                )
            }
            "app.save_layout" => match self.save_layout_for_testing() {
                Ok(()) => (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                ),
                Err(error) => (
                    McpResponse::ActionResult {
                        ok: false,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: Some(error),
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                ),
            },
            "app.lifecycle.restart" => match self.perform_test_restart() {
                Ok(task) => (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    task,
                ),
                Err(error) => (
                    McpResponse::ActionResult {
                        ok: false,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: Some(error),
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                ),
            },
            "settings.open" => {
                self.settings_open = true;
                if let Some(tab) = self.string_arg(args, "tab") {
                    self.settings_tab = tab;
                }
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            "settings.close" => {
                self.settings_open = false;
                self.shortcut_capturing_index = None;
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            "settings.shortcuts.badge.click" => {
                let index = args
                    .and_then(|v| v.get("index"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                self.shortcut_capturing_index = Some(index);
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            "settings.shortcuts.badge.cancel_capture" => {
                self.shortcut_capturing_index = None;
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            "layout.split.create" => {
                let direction = self
                    .string_arg(args, "direction")
                    .unwrap_or_else(|| "horizontal".to_string());
                let dir = match direction.as_str() {
                    "vertical" => SplitDirection::Vertical,
                    _ => SplitDirection::Horizontal,
                };
                let Some(workspace_id) = self
                    .resolve_workspace_id_from_args(args, true, true)
                    .or_else(|| self.workspaces.active_id().map(str::to_string))
                else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("No active workspace".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                let Some(workspace) = self.workspaces.get_mut(&workspace_id) else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some(format!("Workspace {} not found", workspace_id)),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                let leaf_ids: Vec<String> = workspace.layout.all_leaf_ids().into_iter().map(str::to_string).collect();
                let all_terminals = self.terminals.terminals_for_workspace(&workspace_id);
                let unlisted: Option<String> = all_terminals
                    .iter()
                    .find(|t| !leaf_ids.contains(&t.id))
                    .map(|t| t.id.clone());
                let Some(other_id) = unlisted else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("No unlisted terminal to split with".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                let focused = workspace.focused_terminal.clone();
                workspace.layout.split_leaf(&focused, other_id, dir);
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            "layout.split.clear" => {
                let Some(workspace_id) = self
                    .resolve_workspace_id_from_args(args, true, true)
                    .or_else(|| self.workspaces.active_id().map(str::to_string))
                else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("No active workspace".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                if let Some(workspace) = self.workspaces.get_mut(&workspace_id) {
                    let focused = workspace.focused_terminal.clone();
                    workspace.layout = LayoutNode::Leaf {
                        terminal_id: focused,
                    };
                }
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            "pane.focus.next" => {
                let Some(workspace_id) = self.workspaces.active_id().map(str::to_string) else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("No active workspace".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                let Some(workspace) = self.workspaces.get_mut(&workspace_id) else {
                    return (
                        McpResponse::ActionResult {
                            ok: false,
                            target: target.to_string(),
                            action: action.to_string(),
                            error: Some("Workspace not found".to_string()),
                            timestamp_ms: Self::now_ms(),
                        },
                        iced::Task::none(),
                    );
                };
                let current = workspace.focused_terminal.clone();
                if let Some(next) = workspace.layout.next_leaf_id(&current) {
                    workspace.focused_terminal = next.to_string();
                    self.terminals.set_active(&workspace.focused_terminal);
                }
                (
                    McpResponse::ActionResult {
                        ok: true,
                        target: target.to_string(),
                        action: action.to_string(),
                        error: None,
                        timestamp_ms: Self::now_ms(),
                    },
                    iced::Task::none(),
                )
            }
            _ => (
                McpResponse::ActionResult {
                    ok: false,
                    target: target.to_string(),
                    action: action.to_string(),
                    error: Some(format!("Unknown action: {}", key)),
                    timestamp_ms: Self::now_ms(),
                },
                iced::Task::none(),
            ),
        }
    }

    fn handle_ui_wait(&self, condition: &str, args: Option<&Value>) -> McpResponse {
        match self.check_ui_condition(condition, args) {
            Ok(ok) => McpResponse::WaitCompleted {
                ok,
                condition: condition.to_string(),
                timed_out: false,
                elapsed_ms: 0,
                error: None,
            },
            Err(error) => McpResponse::WaitCompleted {
                ok: false,
                condition: condition.to_string(),
                timed_out: false,
                elapsed_ms: 0,
                error: Some(error),
            },
        }
    }

    fn resolve_ui_query(&self, target: &str, args: Option<&Value>) -> Result<Value, String> {
        match target {
            "workspace.active" => Ok(self
                .workspaces
                .active()
                .map(Self::workspace_json)
                .unwrap_or(Value::Null)),
            "workspace.details" => {
                let workspace_id = self.resolve_workspace_id_from_args(args, true, true);
                let workspace = workspace_id
                    .as_deref()
                    .and_then(|id| self.workspaces.get(id));
                Ok(workspace
                    .map(Self::workspace_json)
                    .unwrap_or(Value::Null))
            }
            "workspace.list" => Ok(Value::Array(
                self.workspaces.iter().map(Self::workspace_json).collect(),
            )),
            "tab.active" => Ok(self
                .terminals
                .active_id()
                .map(|id| Value::String(id.to_string()))
                .unwrap_or(Value::Null)),
            "tab.list" => {
                let workspace_id = self.resolve_workspace_id_from_args(args, true, true);
                let terminals = workspace_id
                    .as_deref()
                    .map(|id| self.terminals.terminals_for_workspace(id))
                    .unwrap_or_default();
                Ok(Value::Array(
                    terminals
                        .into_iter()
                        .map(|terminal| Value::String(terminal.id.clone()))
                        .collect(),
                ))
            }
            "pane.active" => Ok(self
                .workspaces
                .active()
                .map(|workspace| Value::String(workspace.focused_terminal.clone()))
                .unwrap_or(Value::Null)),
            "layout.tree" => {
                let workspace_id = self
                    .resolve_workspace_id_from_args(args, true, true)
                    .ok_or_else(|| "No active workspace".to_string())?;
                let workspace = self
                    .workspaces
                    .get(&workspace_id)
                    .ok_or_else(|| format!("Workspace {} not found", workspace_id))?;
                serde_json::to_value(PersistedLayoutNode::from_layout(&workspace.layout))
                    .map_err(|error| format!("Failed to serialize layout tree: {}", error))
            }
            "terminal.count" => {
                let workspace_id = self.resolve_workspace_id_from_args(args, true, true);
                let count = workspace_id
                    .as_deref()
                    .map(|id| self.terminals.terminals_for_workspace(id).len())
                    .unwrap_or(0);
                Ok(json!(count))
            }
            "terminal.list" => {
                let workspace_id = self.resolve_workspace_id_from_args(args, true, true);
                let terminals = workspace_id
                    .as_deref()
                    .map(|id| self.terminals.terminals_for_workspace(id))
                    .unwrap_or_default();
                Ok(Value::Array(
                    terminals
                        .into_iter()
                        .map(Self::terminal_json)
                        .collect(),
                ))
            }
            "terminal.cwd" => {
                let terminal_id = self
                    .resolve_terminal_id_from_args(args, true)
                    .or_else(|| self.terminals.active_id().map(str::to_string));
                let workspace_id = terminal_id
                    .as_deref()
                    .and_then(|id| self.find_workspace_for_terminal(id));
                let folder_path = workspace_id
                    .as_deref()
                    .and_then(|id| self.workspaces.get(id))
                    .map(|ws| ws.folder_path.as_str())
                    .unwrap_or(".");
                Ok(Value::String(folder_path.to_string()))
            }
            "settings.shortcuts.badge" => {
                let index = args
                    .and_then(|v| v.get("index"))
                    .and_then(Value::as_u64)
                    .unwrap_or(0) as usize;
                match shortcuts_tab::get_badge_info(index, self.shortcut_capturing_index, &self.shortcut_overrides) {
                    Some((action, text, capturing)) => Ok(json!({
                        "action": action,
                        "text": text,
                        "capturing": capturing,
                        "clickable": true,
                        "cursor": "pointer",
                    })),
                    None => Err(format!("Badge index {} out of range", index)),
                }
            }
            _ => Err(format!("Unknown query target: {}", target)),
        }
    }

    fn check_ui_condition(&self, condition: &str, args: Option<&Value>) -> Result<bool, String> {
        match condition {
            "app.ready" => Ok(self.workspaces.count() > 0 && self.terminals.count() > 0),
            "settings.visible" => Ok(self.settings_open),
            "terminal.created" => {
                let terminal_id = self.resolve_terminal_id_from_args(args, true);
                Ok(terminal_id
                    .as_deref()
                    .map(|id| self.terminals.get(id).is_some())
                    .unwrap_or_else(|| self.terminals.active_id().is_some()))
            }
            "workspace.switched" => {
                let expected = self.resolve_workspace_id_from_args(args, true, false);
                Ok(match (self.workspaces.active_id(), expected.as_deref()) {
                    (Some(active_id), Some(expected_id)) => active_id == expected_id,
                    (Some(active_id), None) => self
                        .last_test_workspace_id
                        .as_deref()
                        .map(|expected_id| expected_id == active_id)
                        .unwrap_or(false),
                    _ => false,
                })
            }
            "terminal.idle" => {
                let terminal_id = self
                    .resolve_terminal_id_from_args(args, true)
                    .or_else(|| self.terminals.active_id().map(str::to_string));
                Ok(terminal_id
                    .as_deref()
                    .and_then(|id| self.terminals.get(id))
                    .map(|t| !t.fetching && t.grid.is_some())
                    .unwrap_or(false))
            }
            "terminal.count" => {
                let Some(expected) = args.and_then(|value| value.get("count")).and_then(Value::as_u64) else {
                    return Err("count required".to_string());
                };
                let workspace_id = self
                    .resolve_workspace_id_from_args(args, true, true)
                    .ok_or_else(|| "No active workspace".to_string())?;
                Ok(self.terminals.terminals_for_workspace(&workspace_id).len() == expected as usize)
            }
            _ => Err(format!("Unknown condition: {}", condition)),
        }
    }

    fn resolve_workspace_id_from_args(
        &self,
        args: Option<&Value>,
        allow_last: bool,
        fallback_active: bool,
    ) -> Option<String> {
        let explicit = self.string_arg(args, "workspace_id");
        if explicit.is_some() {
            return explicit;
        }
        // Resolve by name if provided
        if let Some(name) = self.string_arg(args, "name") {
            if let Some(workspace) = self.workspaces.iter().find(|ws| ws.name == name) {
                return Some(workspace.id.clone());
            }
        }
        if allow_last && self.bool_arg(args, "use_last") {
            if let Some(workspace_id) = self.last_test_workspace_id.clone() {
                return Some(workspace_id);
            }
            if let Some(workspace) = self.workspaces.iter().last() {
                return Some(workspace.id.clone());
            }
        }
        if fallback_active {
            return self.workspaces.active_id().map(str::to_string);
        }
        None
    }

    fn resolve_terminal_id_from_args(
        &self,
        args: Option<&Value>,
        allow_last: bool,
    ) -> Option<String> {
        let explicit = self.string_arg(args, "terminal_id");
        if explicit.is_some() {
            return explicit;
        }
        if allow_last && self.bool_arg(args, "use_last") {
            if let Some(terminal_id) = self.last_test_terminal_id.clone() {
                return Some(terminal_id);
            }
        }
        None
    }

    fn string_arg(&self, args: Option<&Value>, key: &str) -> Option<String> {
        args.and_then(|value| value.get(key))
            .and_then(Value::as_str)
            .map(str::to_string)
    }

    fn bool_arg(&self, args: Option<&Value>, key: &str) -> bool {
        args.and_then(|value| value.get(key))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }

    fn build_state_dump_json(&self) -> Value {
        let workspaces = Value::Array(
            self.workspaces.iter().map(Self::workspace_json).collect(),
        );
        let terminals = Value::Array(
            self.terminals.iter().map(Self::terminal_json).collect(),
        );
        let layout_trees = Value::Object(
            self.workspaces
                .iter()
                .map(|workspace| {
                    let value = serde_json::to_value(PersistedLayoutNode::from_layout(&workspace.layout))
                        .unwrap_or(Value::Null);
                    (workspace.id.clone(), value)
                })
                .collect(),
        );
        let dump = StateDump {
            workspaces,
            terminals,
            layout_trees,
            daemon_sessions: Value::Null,
            active_workspace_id: self.workspaces.active_id().map(str::to_string),
            active_terminal_id: self.terminals.active_id().map(str::to_string),
        };
        serde_json::to_value(dump).unwrap_or_else(|_| json!({}))
    }

    fn to_mcp_workspace_info(workspace: &crate::workspace_state::WorkspaceInfo) -> McpWorkspaceInfo {
        McpWorkspaceInfo {
            id: workspace.id.clone(),
            name: workspace.name.clone(),
            folder_path: workspace.folder_path.clone(),
        }
    }

    fn to_mcp_terminal_info(
        &self,
        terminal: &crate::terminal_state::TerminalInfo,
    ) -> McpTerminalInfo {
        McpTerminalInfo {
            id: terminal.id.clone(),
            workspace_id: terminal.workspace_id.clone().unwrap_or_default(),
            name: terminal.tab_label().to_string(),
            process_name: terminal.process_name.clone(),
            exited: terminal.exited,
            exit_code: terminal.exit_code,
        }
    }

    fn workspace_json(workspace: &crate::workspace_state::WorkspaceInfo) -> Value {
        json!({
            "id": workspace.id,
            "name": workspace.name,
            "folder_path": workspace.folder_path,
        })
    }

    fn terminal_json(terminal: &crate::terminal_state::TerminalInfo) -> Value {
        json!({
            "id": terminal.id,
            "workspace_id": terminal.workspace_id,
            "name": terminal.tab_label(),
            "process_name": terminal.process_name,
            "exited": terminal.exited,
            "exit_code": terminal.exit_code,
        })
    }

    /// Find which workspace contains a given terminal ID by searching layout trees.
    fn find_workspace_for_terminal(&self, terminal_id: &str) -> Option<String> {
        for ws in self.workspaces.iter() {
            if ws.layout.find_leaf(terminal_id) {
                return Some(ws.id.clone());
            }
        }
        None
    }
}

fn default_workspace_folder() -> String {
    std::env::current_dir()
        .ok()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| ".".to_string())
}

/// Swap two leaf terminal IDs in a layout tree using a three-step rename.
fn swap_leaves_in_layout(
    node: &mut godly_layout_core::LayoutNode,
    id_a: &str,
    id_b: &str,
) {
    let placeholder = format!("__swap_{}_{}", id_a, id_b);
    rename_leaf(node, id_a, &placeholder);
    rename_leaf(node, id_b, id_a);
    rename_leaf(node, &placeholder, id_b);
}

fn rename_leaf(node: &mut godly_layout_core::LayoutNode, from: &str, to: &str) {
    match node {
        godly_layout_core::LayoutNode::Leaf { terminal_id } => {
            if terminal_id == from {
                *terminal_id = to.to_string();
            }
        }
        godly_layout_core::LayoutNode::Split { first, second, .. } => {
            rename_leaf(first, from, to);
            rename_leaf(second, from, to);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use godly_layout_core::LayoutNode;

    #[test]
    fn swap_leaves_in_layout_swaps_ids() {
        let mut layout = LayoutNode::Split {
            direction: godly_layout_core::SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t2".into(),
            }),
        };

        swap_leaves_in_layout(&mut layout, "t1", "t2");
        assert_eq!(layout.all_leaf_ids(), vec!["t2", "t1"]);
    }

    #[test]
    fn swap_leaves_nested_layout() {
        let mut layout = LayoutNode::Split {
            direction: godly_layout_core::SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: godly_layout_core::SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".into(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t3".into(),
                }),
            }),
        };

        swap_leaves_in_layout(&mut layout, "t1", "t3");
        assert_eq!(layout.all_leaf_ids(), vec!["t3", "t2", "t1"]);
    }
}
