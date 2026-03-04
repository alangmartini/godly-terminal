#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSelectionInput {
    pub workspace_id: String,
    pub focused_terminal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSelectionDecision {
    pub workspace_id: String,
    pub clear_context_menu: bool,
    pub mark_terminal_read_id: Option<String>,
}

pub fn reduce_workspace_selection(input: WorkspaceSelectionInput) -> WorkspaceSelectionDecision {
    WorkspaceSelectionDecision {
        workspace_id: input.workspace_id,
        clear_context_menu: true,
        mark_terminal_read_id: input.focused_terminal_id,
    }
}

pub fn reduce_workspace_context_toggle(
    current_menu_id: Option<&str>,
    target_id: &str,
) -> Option<String> {
    if current_menu_id == Some(target_id) {
        None
    } else {
        Some(target_id.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorkspaceInput {
    pub workspace_id: String,
    pub session_id: String,
    pub next_workspace_num: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorkspaceDecision {
    pub workspace_id: String,
    pub session_id: String,
    pub workspace_name: String,
    pub next_workspace_num: u32,
}

pub fn reduce_new_workspace(input: NewWorkspaceInput) -> NewWorkspaceDecision {
    NewWorkspaceDecision {
        workspace_name: format!("Workspace {}", input.next_workspace_num),
        next_workspace_num: input.next_workspace_num + 1,
        workspace_id: input.workspace_id,
        session_id: input.session_id,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteWorkspaceInput {
    pub workspace_count: usize,
    pub terminal_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeleteWorkspaceDecision {
    RejectedLastWorkspace,
    Delete {
        terminal_ids: Vec<String>,
        clear_context_menu: bool,
    },
}

pub fn reduce_delete_workspace(input: DeleteWorkspaceInput) -> DeleteWorkspaceDecision {
    if input.workspace_count <= 1 {
        DeleteWorkspaceDecision::RejectedLastWorkspace
    } else {
        DeleteWorkspaceDecision::Delete {
            terminal_ids: input.terminal_ids,
            clear_context_menu: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostWorkspaceDeleteDecision {
    pub mark_terminal_read_id: Option<String>,
    pub fetch_grid_terminal_id: Option<String>,
}

pub fn reduce_post_workspace_delete(
    focused_terminal_id: Option<String>,
) -> PostWorkspaceDeleteDecision {
    PostWorkspaceDeleteDecision {
        fetch_grid_terminal_id: focused_terminal_id.clone(),
        mark_terminal_read_id: focused_terminal_id,
    }
}

pub fn reduce_workspace_switch_read_target(focused_terminal_id: Option<String>) -> Option<String> {
    focused_terminal_id
}

#[cfg(test)]
mod tests {
    use super::{
        reduce_delete_workspace, reduce_new_workspace, reduce_post_workspace_delete,
        reduce_workspace_context_toggle, reduce_workspace_selection,
        reduce_workspace_switch_read_target, DeleteWorkspaceDecision, DeleteWorkspaceInput,
        NewWorkspaceInput, WorkspaceSelectionInput,
    };

    #[test]
    fn workspace_selection_clears_context_and_marks_focus_as_read() {
        let decision = reduce_workspace_selection(WorkspaceSelectionInput {
            workspace_id: "w-2".into(),
            focused_terminal_id: Some("t-2".into()),
        });

        assert_eq!(decision.workspace_id, "w-2");
        assert!(decision.clear_context_menu);
        assert_eq!(decision.mark_terminal_read_id.as_deref(), Some("t-2"));
    }

    #[test]
    fn workspace_context_menu_toggle_closes_when_reselected() {
        assert_eq!(reduce_workspace_context_toggle(Some("w-1"), "w-1"), None);
        assert_eq!(
            reduce_workspace_context_toggle(Some("w-1"), "w-2"),
            Some("w-2".into())
        );
    }

    #[test]
    fn new_workspace_uses_counter_for_name_and_increments_it() {
        let decision = reduce_new_workspace(NewWorkspaceInput {
            workspace_id: "w-3".into(),
            session_id: "s-3".into(),
            next_workspace_num: 7,
        });

        assert_eq!(decision.workspace_name, "Workspace 7");
        assert_eq!(decision.next_workspace_num, 8);
        assert_eq!(decision.workspace_id, "w-3");
        assert_eq!(decision.session_id, "s-3");
    }

    #[test]
    fn delete_workspace_rejects_last_workspace() {
        let decision = reduce_delete_workspace(DeleteWorkspaceInput {
            workspace_count: 1,
            terminal_ids: vec!["t-1".into()],
        });
        assert_eq!(decision, DeleteWorkspaceDecision::RejectedLastWorkspace);
    }

    #[test]
    fn delete_workspace_returns_terminal_cleanup_plan() {
        let decision = reduce_delete_workspace(DeleteWorkspaceInput {
            workspace_count: 3,
            terminal_ids: vec!["t-1".into(), "t-2".into()],
        });
        assert_eq!(
            decision,
            DeleteWorkspaceDecision::Delete {
                terminal_ids: vec!["t-1".into(), "t-2".into()],
                clear_context_menu: true
            }
        );
    }

    #[test]
    fn post_delete_decision_drives_read_and_fetch_targets() {
        let decision = reduce_post_workspace_delete(Some("t-next".into()));
        assert_eq!(decision.mark_terminal_read_id.as_deref(), Some("t-next"));
        assert_eq!(decision.fetch_grid_terminal_id.as_deref(), Some("t-next"));

        let none = reduce_post_workspace_delete(None);
        assert_eq!(none.mark_terminal_read_id, None);
        assert_eq!(none.fetch_grid_terminal_id, None);
    }

    #[test]
    fn switch_read_target_passes_through_focus() {
        assert_eq!(
            reduce_workspace_switch_read_target(Some("t-1".into())),
            Some("t-1".into())
        );
        assert_eq!(reduce_workspace_switch_read_target(None), None);
    }
}
