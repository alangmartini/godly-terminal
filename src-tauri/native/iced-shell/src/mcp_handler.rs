use godly_app_adapter::mcp_pipe::McpEvent;
use godly_layout_core::SplitDirection;

use crate::app::{GodlyApp, Message};

impl GodlyApp {
    /// Handle an incoming MCP event by mutating app state and returning any
    /// follow-up tasks (e.g., grid fetch, terminal creation).
    pub(crate) fn handle_mcp_event(&mut self, event: McpEvent) -> iced::Task<Message> {
        match event {
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
                iced::Task::none()
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
                iced::Task::none()
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
