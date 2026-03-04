#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabClickInput {
    pub terminal_id: String,
    pub terminal_in_active_layout: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabClickDecision {
    pub activate_terminal_id: String,
    pub focus_workspace_terminal_id: Option<String>,
    pub mark_terminal_read_id: String,
}

pub fn reduce_tab_click(input: TabClickInput) -> TabClickDecision {
    let focus_workspace_terminal_id = input
        .terminal_in_active_layout
        .then(|| input.terminal_id.clone());

    TabClickDecision {
        activate_terminal_id: input.terminal_id.clone(),
        mark_terminal_read_id: input.terminal_id,
        focus_workspace_terminal_id,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCreatedInput {
    pub session_id: String,
    pub active_workspace_id: Option<String>,
    pub terminal_in_active_layout: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkspaceTerminalMutation {
    FocusTerminal,
    ResetLayoutToSingleTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalCreatedDecision {
    pub session_id: String,
    pub assign_workspace_id: Option<String>,
    pub workspace_mutation: Option<WorkspaceTerminalMutation>,
    pub set_terminal_active: bool,
    pub fetch_grid_terminal_id: String,
}

pub fn reduce_terminal_created(input: TerminalCreatedInput) -> TerminalCreatedDecision {
    let (workspace_mutation, set_terminal_active) = if input.terminal_in_active_layout {
        (Some(WorkspaceTerminalMutation::FocusTerminal), false)
    } else if input.active_workspace_id.is_some() {
        (
            Some(WorkspaceTerminalMutation::ResetLayoutToSingleTerminal),
            true,
        )
    } else {
        (None, true)
    };

    TerminalCreatedDecision {
        fetch_grid_terminal_id: input.session_id.clone(),
        session_id: input.session_id,
        assign_workspace_id: input.active_workspace_id,
        workspace_mutation,
        set_terminal_active,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        reduce_tab_click, reduce_terminal_created, TabClickInput, TerminalCreatedInput,
        WorkspaceTerminalMutation,
    };

    #[test]
    fn tab_click_in_layout_updates_workspace_focus() {
        let decision = reduce_tab_click(TabClickInput {
            terminal_id: "t-1".into(),
            terminal_in_active_layout: true,
        });

        assert_eq!(decision.activate_terminal_id, "t-1");
        assert_eq!(decision.mark_terminal_read_id, "t-1");
        assert_eq!(decision.focus_workspace_terminal_id.as_deref(), Some("t-1"));
    }

    #[test]
    fn tab_click_outside_layout_keeps_workspace_focus_unchanged() {
        let decision = reduce_tab_click(TabClickInput {
            terminal_id: "t-2".into(),
            terminal_in_active_layout: false,
        });

        assert_eq!(decision.activate_terminal_id, "t-2");
        assert_eq!(decision.mark_terminal_read_id, "t-2");
        assert_eq!(decision.focus_workspace_terminal_id, None);
    }

    #[test]
    fn terminal_created_for_split_only_moves_workspace_focus() {
        let decision = reduce_terminal_created(TerminalCreatedInput {
            session_id: "split-child".into(),
            active_workspace_id: Some("w-1".into()),
            terminal_in_active_layout: true,
        });

        assert_eq!(decision.session_id, "split-child");
        assert_eq!(decision.assign_workspace_id.as_deref(), Some("w-1"));
        assert_eq!(
            decision.workspace_mutation,
            Some(WorkspaceTerminalMutation::FocusTerminal)
        );
        assert!(!decision.set_terminal_active);
        assert_eq!(decision.fetch_grid_terminal_id, "split-child");
    }

    #[test]
    fn terminal_created_as_regular_tab_resets_workspace_layout() {
        let decision = reduce_terminal_created(TerminalCreatedInput {
            session_id: "new-tab".into(),
            active_workspace_id: Some("w-1".into()),
            terminal_in_active_layout: false,
        });

        assert_eq!(decision.assign_workspace_id.as_deref(), Some("w-1"));
        assert_eq!(
            decision.workspace_mutation,
            Some(WorkspaceTerminalMutation::ResetLayoutToSingleTerminal)
        );
        assert!(decision.set_terminal_active);
    }

    #[test]
    fn terminal_created_without_active_workspace_only_activates_tab() {
        let decision = reduce_terminal_created(TerminalCreatedInput {
            session_id: "orphan".into(),
            active_workspace_id: None,
            terminal_in_active_layout: false,
        });

        assert_eq!(decision.assign_workspace_id, None);
        assert_eq!(decision.workspace_mutation, None);
        assert!(decision.set_terminal_active);
    }
}
