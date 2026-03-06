use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabMruTouchInput {
    pub mru_terminal_ids: Vec<String>,
    pub terminal_id: String,
}

pub fn reduce_tab_mru_touch(input: TabMruTouchInput) -> Vec<String> {
    if input.terminal_id.trim().is_empty() {
        return input.mru_terminal_ids;
    }

    let mut next_mru = Vec::with_capacity(input.mru_terminal_ids.len() + 1);
    next_mru.push(input.terminal_id.clone());
    for id in input.mru_terminal_ids {
        if id != input.terminal_id {
            push_unique(&mut next_mru, id);
        }
    }

    next_mru
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabMruCleanupInput {
    pub mru_terminal_ids: Vec<String>,
    pub open_terminal_ids: Vec<String>,
    pub active_terminal_id: Option<String>,
}

pub fn reduce_tab_mru_cleanup(input: TabMruCleanupInput) -> Vec<String> {
    let open_id_set: HashSet<String> = input.open_terminal_ids.iter().cloned().collect();
    let mut next_mru = Vec::with_capacity(input.open_terminal_ids.len());

    for id in input.mru_terminal_ids {
        if open_id_set.contains(&id) {
            push_unique(&mut next_mru, id);
        }
    }
    for id in input.open_terminal_ids {
        push_unique(&mut next_mru, id);
    }

    if let Some(active_terminal_id) = input.active_terminal_id {
        if let Some(active_index) = next_mru.iter().position(|id| id == &active_terminal_id) {
            let active_id = next_mru.remove(active_index);
            next_mru.insert(0, active_id);
        }
    }

    next_mru
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabMruCycleDirection {
    Forward,
    Backward,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabMruCycleInput {
    pub mru_terminal_ids: Vec<String>,
    pub current_terminal_id: Option<String>,
    pub direction: TabMruCycleDirection,
}

pub fn reduce_tab_mru_cycle(input: TabMruCycleInput) -> Option<String> {
    if input.mru_terminal_ids.is_empty() {
        return None;
    }

    let current_index = input
        .current_terminal_id
        .and_then(|current_id| {
            input
                .mru_terminal_ids
                .iter()
                .position(|id| id == &current_id)
        })
        .unwrap_or(0);
    let terminal_count = input.mru_terminal_ids.len();
    let next_index = match input.direction {
        TabMruCycleDirection::Forward => (current_index + 1) % terminal_count,
        TabMruCycleDirection::Backward => {
            if current_index == 0 {
                terminal_count - 1
            } else {
                current_index - 1
            }
        }
    };

    input.mru_terminal_ids.get(next_index).cloned()
}

fn push_unique(ids: &mut Vec<String>, id: String) {
    if !ids.iter().any(|existing| existing == &id) {
        ids.push(id);
    }
}

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

pub fn reduce_tab_context_toggle(current_menu_id: Option<&str>, target_id: &str) -> Option<String> {
    if current_menu_id == Some(target_id) {
        None
    } else {
        Some(target_id.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabRenameInput {
    pub terminal_id: String,
    pub raw_name: String,
    pub terminal_exists: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabRenameDecision {
    pub terminal_id: String,
    pub next_custom_name: Option<String>,
    pub clear_context_menu: bool,
}

pub fn reduce_tab_rename(input: TabRenameInput) -> Option<TabRenameDecision> {
    if !input.terminal_exists || input.terminal_id.trim().is_empty() {
        return None;
    }

    let trimmed_name = input.raw_name.trim();
    let next_custom_name = if trimmed_name.is_empty() {
        None
    } else {
        Some(trimmed_name.to_string())
    };

    Some(TabRenameDecision {
        terminal_id: input.terminal_id,
        next_custom_name,
        clear_context_menu: true,
    })
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
    use super::TabMruCycleDirection::{Backward, Forward};
    use super::{
        reduce_tab_click, reduce_tab_context_toggle, reduce_tab_mru_cleanup, reduce_tab_mru_cycle,
        reduce_tab_mru_touch, reduce_tab_rename, reduce_terminal_created, TabClickInput,
        TabMruCleanupInput, TabMruCycleDirection, TabMruCycleInput, TabMruTouchInput,
        TabRenameInput, TerminalCreatedInput, WorkspaceTerminalMutation,
    };

    #[test]
    fn tab_mru_touch_moves_target_to_front_and_dedupes() {
        let touched = reduce_tab_mru_touch(TabMruTouchInput {
            mru_terminal_ids: vec!["t-1".into(), "t-2".into(), "t-1".into(), "t-3".into()],
            terminal_id: "t-2".into(),
        });
        assert_eq!(touched, vec!["t-2", "t-1", "t-3"]);

        let untouched = reduce_tab_mru_touch(TabMruTouchInput {
            mru_terminal_ids: vec!["t-1".into(), "t-2".into()],
            terminal_id: "  ".into(),
        });
        assert_eq!(untouched, vec!["t-1", "t-2"]);
    }

    #[test]
    fn tab_mru_cleanup_prunes_missing_ids_and_appends_new_ids() {
        let cleaned = reduce_tab_mru_cleanup(TabMruCleanupInput {
            mru_terminal_ids: vec![
                "stale".into(),
                "t-2".into(),
                "t-2".into(),
                "t-1".into(),
                "ghost".into(),
            ],
            open_terminal_ids: vec!["t-1".into(), "t-2".into(), "t-3".into()],
            active_terminal_id: None,
        });

        assert_eq!(cleaned, vec!["t-2", "t-1", "t-3"]);
    }

    #[test]
    fn tab_mru_cleanup_keeps_active_terminal_first_when_present() {
        let cleaned = reduce_tab_mru_cleanup(TabMruCleanupInput {
            mru_terminal_ids: vec!["t-2".into(), "t-1".into(), "t-3".into()],
            open_terminal_ids: vec!["t-1".into(), "t-2".into(), "t-3".into()],
            active_terminal_id: Some("t-3".into()),
        });

        assert_eq!(cleaned, vec!["t-3", "t-2", "t-1"]);
    }

    #[test]
    fn tab_mru_cycle_wraps_in_both_directions() {
        let mru = vec!["t-1".into(), "t-2".into(), "t-3".into()];

        assert_eq!(
            reduce_tab_mru_cycle(TabMruCycleInput {
                mru_terminal_ids: mru.clone(),
                current_terminal_id: Some("t-1".into()),
                direction: Forward,
            }),
            Some("t-2".into())
        );
        assert_eq!(
            reduce_tab_mru_cycle(TabMruCycleInput {
                mru_terminal_ids: mru.clone(),
                current_terminal_id: Some("t-1".into()),
                direction: Backward,
            }),
            Some("t-3".into())
        );
        assert_eq!(
            reduce_tab_mru_cycle(TabMruCycleInput {
                mru_terminal_ids: mru.clone(),
                current_terminal_id: Some("missing".into()),
                direction: TabMruCycleDirection::Forward,
            }),
            Some("t-2".into())
        );
        assert_eq!(
            reduce_tab_mru_cycle(TabMruCycleInput {
                mru_terminal_ids: Vec::new(),
                current_terminal_id: Some("t-1".into()),
                direction: Forward,
            }),
            None
        );
    }

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

    #[test]
    fn tab_context_menu_toggle_closes_when_reselected() {
        assert_eq!(reduce_tab_context_toggle(Some("t-1"), "t-1"), None);
        assert_eq!(
            reduce_tab_context_toggle(Some("t-1"), "t-2"),
            Some("t-2".into())
        );
    }

    #[test]
    fn tab_rename_rejects_missing_terminal() {
        let decision = reduce_tab_rename(TabRenameInput {
            terminal_id: "t-1".into(),
            raw_name: "Build".into(),
            terminal_exists: false,
        });

        assert_eq!(decision, None);
    }

    #[test]
    fn tab_rename_rejects_empty_terminal_id() {
        let decision = reduce_tab_rename(TabRenameInput {
            terminal_id: "".into(),
            raw_name: "Build".into(),
            terminal_exists: true,
        });

        assert_eq!(decision, None);
    }

    #[test]
    fn tab_rename_trims_name_and_clears_context_menu() {
        let decision = reduce_tab_rename(TabRenameInput {
            terminal_id: "t-1".into(),
            raw_name: "  Build Logs  ".into(),
            terminal_exists: true,
        });

        assert_eq!(
            decision,
            Some(super::TabRenameDecision {
                terminal_id: "t-1".into(),
                next_custom_name: Some("Build Logs".into()),
                clear_context_menu: true,
            })
        );
    }

    #[test]
    fn tab_rename_with_blank_name_clears_custom_name() {
        let decision = reduce_tab_rename(TabRenameInput {
            terminal_id: "t-2".into(),
            raw_name: "   ".into(),
            terminal_exists: true,
        });

        assert_eq!(
            decision,
            Some(super::TabRenameDecision {
                terminal_id: "t-2".into(),
                next_custom_name: None,
                clear_context_menu: true,
            })
        );
    }
}
