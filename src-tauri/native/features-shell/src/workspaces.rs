use godly_layout_core::{LayoutNode, SplitDirection};

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

const CROSS_WORKSPACE_MOVE_SPLIT_DIRECTION: SplitDirection = SplitDirection::Horizontal;

#[derive(Debug, Clone, PartialEq)]
pub struct MoveTerminalAcrossWorkspacesInput {
    pub source_workspace_id: String,
    pub target_workspace_id: String,
    pub moved_terminal_id: String,
    pub source_layout: Option<LayoutNode>,
    pub target_layout: Option<LayoutNode>,
    pub source_focused_terminal_id: Option<String>,
    pub target_focused_terminal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MoveTerminalAcrossWorkspacesDecision {
    RejectedSameWorkspace,
    RejectedMissingSourceLayout,
    RejectedMissingTargetLayout,
    RejectedMissingMovedTerminalInSourceLayout,
    RejectedLastSourceLeaf,
    RejectedInvalidSourceLayout,
    RejectedInvalidTargetLayout,
    Move {
        source_workspace_id: String,
        target_workspace_id: String,
        moved_terminal_id: String,
        next_source_layout: LayoutNode,
        next_target_layout: LayoutNode,
        next_source_focused_terminal_id: String,
        next_target_focused_terminal_id: String,
    },
}

pub fn reduce_move_terminal_across_workspaces(
    input: MoveTerminalAcrossWorkspacesInput,
) -> MoveTerminalAcrossWorkspacesDecision {
    if input.source_workspace_id == input.target_workspace_id {
        return MoveTerminalAcrossWorkspacesDecision::RejectedSameWorkspace;
    }

    let mut source_layout = match input.source_layout {
        Some(layout) => layout,
        None => return MoveTerminalAcrossWorkspacesDecision::RejectedMissingSourceLayout,
    };
    let mut target_layout = match input.target_layout {
        Some(layout) => layout,
        None => return MoveTerminalAcrossWorkspacesDecision::RejectedMissingTargetLayout,
    };

    if !source_layout.find_leaf(&input.moved_terminal_id) {
        return MoveTerminalAcrossWorkspacesDecision::RejectedMissingMovedTerminalInSourceLayout;
    }

    if source_layout.leaf_count() <= 1 {
        return MoveTerminalAcrossWorkspacesDecision::RejectedLastSourceLeaf;
    }

    if source_layout
        .unsplit_leaf(&input.moved_terminal_id)
        .is_none()
    {
        return MoveTerminalAcrossWorkspacesDecision::RejectedInvalidSourceLayout;
    }

    let next_source_focused_terminal_id = match input.source_focused_terminal_id {
        Some(focused_terminal_id) if source_layout.find_leaf(&focused_terminal_id) => {
            focused_terminal_id
        }
        _ => match first_leaf_id(&source_layout) {
            Some(terminal_id) => terminal_id,
            None => return MoveTerminalAcrossWorkspacesDecision::RejectedInvalidSourceLayout,
        },
    };

    let split_target_terminal_id = input
        .target_focused_terminal_id
        .filter(|focused_terminal_id| target_layout.find_leaf(focused_terminal_id))
        .or_else(|| first_leaf_id(&target_layout));
    let Some(split_target_terminal_id) = split_target_terminal_id else {
        return MoveTerminalAcrossWorkspacesDecision::RejectedInvalidTargetLayout;
    };

    if !target_layout.split_leaf(
        &split_target_terminal_id,
        input.moved_terminal_id.clone(),
        CROSS_WORKSPACE_MOVE_SPLIT_DIRECTION,
    ) {
        return MoveTerminalAcrossWorkspacesDecision::RejectedInvalidTargetLayout;
    }

    MoveTerminalAcrossWorkspacesDecision::Move {
        source_workspace_id: input.source_workspace_id,
        target_workspace_id: input.target_workspace_id,
        moved_terminal_id: input.moved_terminal_id.clone(),
        next_source_layout: source_layout,
        next_target_layout: target_layout,
        next_source_focused_terminal_id,
        next_target_focused_terminal_id: input.moved_terminal_id,
    }
}

fn first_leaf_id(layout: &LayoutNode) -> Option<String> {
    layout
        .all_leaf_ids()
        .first()
        .map(|terminal_id| terminal_id.to_string())
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
        reduce_delete_workspace, reduce_move_terminal_across_workspaces, reduce_new_workspace,
        reduce_post_workspace_delete, reduce_workspace_context_toggle, reduce_workspace_selection,
        reduce_workspace_switch_read_target, DeleteWorkspaceDecision, DeleteWorkspaceInput,
        MoveTerminalAcrossWorkspacesDecision, MoveTerminalAcrossWorkspacesInput, NewWorkspaceInput,
        WorkspaceSelectionInput,
    };
    use godly_layout_core::{LayoutNode, SplitDirection};

    fn leaf(id: &str) -> LayoutNode {
        LayoutNode::Leaf {
            terminal_id: id.to_string(),
        }
    }

    fn split(first: &str, second: &str, direction: SplitDirection) -> LayoutNode {
        LayoutNode::Split {
            direction,
            ratio: 0.5,
            first: Box::new(leaf(first)),
            second: Box::new(leaf(second)),
        }
    }

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
    fn move_across_workspaces_rejects_same_workspace() {
        let decision = reduce_move_terminal_across_workspaces(MoveTerminalAcrossWorkspacesInput {
            source_workspace_id: "w-1".into(),
            target_workspace_id: "w-1".into(),
            moved_terminal_id: "t-1".into(),
            source_layout: Some(split("t-1", "t-2", SplitDirection::Vertical)),
            target_layout: Some(leaf("t-3")),
            source_focused_terminal_id: Some("t-1".into()),
            target_focused_terminal_id: Some("t-3".into()),
        });

        assert_eq!(
            decision,
            MoveTerminalAcrossWorkspacesDecision::RejectedSameWorkspace
        );
    }

    #[test]
    fn move_across_workspaces_rejects_missing_source_layout() {
        let decision = reduce_move_terminal_across_workspaces(MoveTerminalAcrossWorkspacesInput {
            source_workspace_id: "w-1".into(),
            target_workspace_id: "w-2".into(),
            moved_terminal_id: "t-1".into(),
            source_layout: None,
            target_layout: Some(leaf("t-3")),
            source_focused_terminal_id: Some("t-1".into()),
            target_focused_terminal_id: Some("t-3".into()),
        });

        assert_eq!(
            decision,
            MoveTerminalAcrossWorkspacesDecision::RejectedMissingSourceLayout
        );
    }

    #[test]
    fn move_across_workspaces_rejects_missing_target_layout() {
        let decision = reduce_move_terminal_across_workspaces(MoveTerminalAcrossWorkspacesInput {
            source_workspace_id: "w-1".into(),
            target_workspace_id: "w-2".into(),
            moved_terminal_id: "t-1".into(),
            source_layout: Some(split("t-1", "t-2", SplitDirection::Vertical)),
            target_layout: None,
            source_focused_terminal_id: Some("t-1".into()),
            target_focused_terminal_id: Some("t-3".into()),
        });

        assert_eq!(
            decision,
            MoveTerminalAcrossWorkspacesDecision::RejectedMissingTargetLayout
        );
    }

    #[test]
    fn move_across_workspaces_rejects_when_moved_terminal_missing_from_source_layout() {
        let decision = reduce_move_terminal_across_workspaces(MoveTerminalAcrossWorkspacesInput {
            source_workspace_id: "w-1".into(),
            target_workspace_id: "w-2".into(),
            moved_terminal_id: "t-missing".into(),
            source_layout: Some(split("t-1", "t-2", SplitDirection::Vertical)),
            target_layout: Some(leaf("t-3")),
            source_focused_terminal_id: Some("t-1".into()),
            target_focused_terminal_id: Some("t-3".into()),
        });

        assert_eq!(
            decision,
            MoveTerminalAcrossWorkspacesDecision::RejectedMissingMovedTerminalInSourceLayout
        );
    }

    #[test]
    fn move_across_workspaces_rejects_last_source_leaf() {
        let decision = reduce_move_terminal_across_workspaces(MoveTerminalAcrossWorkspacesInput {
            source_workspace_id: "w-1".into(),
            target_workspace_id: "w-2".into(),
            moved_terminal_id: "t-1".into(),
            source_layout: Some(leaf("t-1")),
            target_layout: Some(leaf("t-3")),
            source_focused_terminal_id: Some("t-1".into()),
            target_focused_terminal_id: Some("t-3".into()),
        });

        assert_eq!(
            decision,
            MoveTerminalAcrossWorkspacesDecision::RejectedLastSourceLeaf
        );
    }

    #[test]
    fn move_across_workspaces_moves_layouts_and_keeps_valid_source_focus() {
        let decision = reduce_move_terminal_across_workspaces(MoveTerminalAcrossWorkspacesInput {
            source_workspace_id: "w-1".into(),
            target_workspace_id: "w-2".into(),
            moved_terminal_id: "t-2".into(),
            source_layout: Some(split("t-1", "t-2", SplitDirection::Vertical)),
            target_layout: Some(leaf("t-3")),
            source_focused_terminal_id: Some("t-1".into()),
            target_focused_terminal_id: Some("t-3".into()),
        });

        let MoveTerminalAcrossWorkspacesDecision::Move {
            source_workspace_id,
            target_workspace_id,
            moved_terminal_id,
            next_source_layout,
            next_target_layout,
            next_source_focused_terminal_id,
            next_target_focused_terminal_id,
        } = decision
        else {
            panic!("move should be planned");
        };

        assert_eq!(source_workspace_id, "w-1");
        assert_eq!(target_workspace_id, "w-2");
        assert_eq!(moved_terminal_id, "t-2");
        assert_eq!(next_source_layout.all_leaf_ids(), vec!["t-1"]);
        assert_eq!(next_source_focused_terminal_id, "t-1");
        assert_eq!(next_target_layout.all_leaf_ids(), vec!["t-3", "t-2"]);
        assert_eq!(next_target_focused_terminal_id, "t-2");

        match next_target_layout {
            LayoutNode::Split { direction, .. } => {
                assert_eq!(direction, SplitDirection::Horizontal);
            }
            _ => panic!("target should have been split"),
        }
    }

    #[test]
    fn move_across_workspaces_falls_back_to_first_leaf_when_focuses_are_invalid() {
        let decision = reduce_move_terminal_across_workspaces(MoveTerminalAcrossWorkspacesInput {
            source_workspace_id: "w-1".into(),
            target_workspace_id: "w-2".into(),
            moved_terminal_id: "t-1".into(),
            source_layout: Some(split("t-1", "t-2", SplitDirection::Vertical)),
            target_layout: Some(split("t-3", "t-4", SplitDirection::Vertical)),
            source_focused_terminal_id: Some("t-1".into()),
            target_focused_terminal_id: Some("t-missing".into()),
        });

        let MoveTerminalAcrossWorkspacesDecision::Move {
            next_source_layout,
            next_target_layout,
            next_source_focused_terminal_id,
            next_target_focused_terminal_id,
            ..
        } = decision
        else {
            panic!("move should be planned");
        };

        assert_eq!(next_source_layout.all_leaf_ids(), vec!["t-2"]);
        assert_eq!(next_source_focused_terminal_id, "t-2");
        assert_eq!(next_target_layout.all_leaf_ids(), vec!["t-3", "t-1", "t-4"]);
        assert_eq!(next_target_focused_terminal_id, "t-1");
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
