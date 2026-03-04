use godly_features_shell::layout as layout_reducer;
use godly_features_shell::tabs as tab_reducer;
use godly_features_shell::workspaces as workspace_reducer;
use godly_layout_core::{LayoutNode, SplitDirection};
use godly_tabs_core::TabState;
use godly_workspaces_core::{WorkspaceCollection, WorkspaceInfo};

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

fn workspace(id: &str, focused_terminal: &str, layout: LayoutNode) -> WorkspaceInfo<LayoutNode> {
    WorkspaceInfo {
        id: id.to_string(),
        name: format!("Workspace {}", id),
        folder_path: ".".to_string(),
        worktree_mode: false,
        layout,
        focused_terminal: focused_terminal.to_string(),
    }
}

fn apply_terminal_created(
    tabs: &mut TabState,
    workspaces: &mut WorkspaceCollection<LayoutNode>,
    decision: tab_reducer::TerminalCreatedDecision,
) {
    assert!(
        tabs.open(decision.session_id.clone()),
        "session should be inserted once"
    );

    if decision.set_terminal_active {
        assert!(
            tabs.activate(&decision.session_id),
            "new tab should become active when requested"
        );
    }

    if let Some(mutation) = decision.workspace_mutation {
        let workspace = workspaces
            .active_mut()
            .expect("active workspace should exist while applying mutation");

        match mutation {
            tab_reducer::WorkspaceTerminalMutation::FocusTerminal => {
                workspace.focused_terminal = decision.session_id;
            }
            tab_reducer::WorkspaceTerminalMutation::ResetLayoutToSingleTerminal => {
                workspace.layout = leaf(&decision.session_id);
                workspace.focused_terminal = decision.session_id;
            }
        }
    }
}

#[test]
fn new_tab_flow_resets_layout_and_activates_new_tab() {
    let mut tabs = TabState::new();
    assert!(tabs.open("t-1"));
    assert!(tabs.open("t-2"));
    assert!(tabs.activate("t-1"));

    let mut workspaces = WorkspaceCollection::new();
    workspaces.add(workspace(
        "w-1",
        "t-1",
        split("t-1", "t-2", SplitDirection::Horizontal),
    ));

    let decision = tab_reducer::reduce_terminal_created(tab_reducer::TerminalCreatedInput {
        session_id: "t-3".into(),
        active_workspace_id: workspaces.active_id().map(str::to_string),
        terminal_in_active_layout: false,
    });

    apply_terminal_created(&mut tabs, &mut workspaces, decision.clone());

    let active_workspace = workspaces.active().expect("active workspace");
    assert_eq!(decision.assign_workspace_id.as_deref(), Some("w-1"));
    assert_eq!(tabs.active_id(), Some("t-3"));
    assert_eq!(active_workspace.focused_terminal, "t-3");
    assert_eq!(active_workspace.layout.all_leaf_ids(), vec!["t-3"]);
    assert_eq!(decision.fetch_grid_terminal_id, "t-3");
}

#[test]
fn split_flow_keeps_active_tab_but_moves_workspace_focus() {
    let mut tabs = TabState::new();
    assert!(tabs.open("t-1"));
    assert_eq!(tabs.active_id(), Some("t-1"));

    let mut workspaces = WorkspaceCollection::new();
    workspaces.add(workspace("w-1", "t-1", leaf("t-1")));

    let split_decision = layout_reducer::reduce_split_focused(layout_reducer::SplitFocusedInput {
        focused_terminal_id: workspaces.active().map(|ws| ws.focused_terminal.clone()),
        new_terminal_id: "t-2".into(),
        direction: SplitDirection::Vertical,
    })
    .expect("split should be allowed for focused pane");

    let new_terminal_id = split_decision.new_terminal_id.clone();
    {
        let workspace = workspaces.active_mut().expect("active workspace");
        assert!(workspace.layout.split_leaf(
            &split_decision.focused_terminal_id,
            new_terminal_id.clone(),
            split_decision.direction
        ));
    }
    let terminal_in_active_layout = workspaces
        .active()
        .map(|workspace| workspace.layout.find_leaf(&new_terminal_id))
        .unwrap_or(false);

    let created_decision = tab_reducer::reduce_terminal_created(tab_reducer::TerminalCreatedInput {
        session_id: split_decision.new_terminal_id,
        active_workspace_id: workspaces.active_id().map(str::to_string),
        terminal_in_active_layout,
    });

    apply_terminal_created(&mut tabs, &mut workspaces, created_decision);

    let active_workspace = workspaces.active().expect("active workspace");
    assert_eq!(tabs.active_id(), Some("t-1"));
    assert!(tabs.contains("t-2"));
    assert_eq!(active_workspace.focused_terminal, "t-2");
    assert_eq!(active_workspace.layout.all_leaf_ids(), vec!["t-1", "t-2"]);
}

#[test]
fn workspace_switch_flow_reads_target_workspace_focus() {
    let mut workspaces = WorkspaceCollection::new();
    workspaces.add(workspace("w-1", "t-1", leaf("t-1")));
    workspaces.add(workspace("w-2", "t-3", leaf("t-3")));

    let mut workspace_context_menu_id = Some("w-1".to_string());
    let focused_terminal_id = workspaces
        .get("w-2")
        .map(|ws| ws.focused_terminal.clone())
        .or_else(|| workspaces.active().map(|ws| ws.focused_terminal.clone()));

    let decision = workspace_reducer::reduce_workspace_selection(
        workspace_reducer::WorkspaceSelectionInput {
            workspace_id: "w-2".into(),
            focused_terminal_id,
        },
    );

    assert!(workspaces.set_active(&decision.workspace_id));
    if decision.clear_context_menu {
        workspace_context_menu_id = None;
    }

    let read_target =
        workspace_reducer::reduce_workspace_switch_read_target(decision.mark_terminal_read_id);

    assert_eq!(workspaces.active_id(), Some("w-2"));
    assert!(workspace_context_menu_id.is_none());
    assert_eq!(read_target.as_deref(), Some("t-3"));
}

#[test]
fn split_then_workspace_switch_then_delete_flow_keeps_cross_reducer_state_consistent() {
    let mut tabs = TabState::new();
    assert!(tabs.open("t-1"));
    assert!(tabs.open("t-2"));
    assert!(tabs.open("t-3"));
    assert!(tabs.activate("t-2"));

    let mut workspaces = WorkspaceCollection::new();
    workspaces.add(workspace(
        "w-1",
        "t-1",
        split("t-1", "t-2", SplitDirection::Horizontal),
    ));
    workspaces.add(workspace("w-2", "t-3", leaf("t-3")));

    let click = tab_reducer::reduce_tab_click(tab_reducer::TabClickInput {
        terminal_id: "t-2".into(),
        terminal_in_active_layout: true,
    });
    assert!(tabs.activate(&click.activate_terminal_id));
    if let Some(focus_terminal_id) = click.focus_workspace_terminal_id {
        workspaces
            .active_mut()
            .expect("active workspace")
            .focused_terminal = focus_terminal_id;
    }

    let split_decision = layout_reducer::reduce_split_focused(layout_reducer::SplitFocusedInput {
        focused_terminal_id: workspaces.active().map(|ws| ws.focused_terminal.clone()),
        new_terminal_id: "t-4".into(),
        direction: SplitDirection::Vertical,
    })
    .expect("split should be allowed for focused pane");
    {
        let workspace = workspaces.active_mut().expect("active workspace");
        assert!(workspace.layout.split_leaf(
            &split_decision.focused_terminal_id,
            split_decision.new_terminal_id.clone(),
            split_decision.direction
        ));
    }
    let created = tab_reducer::reduce_terminal_created(tab_reducer::TerminalCreatedInput {
        session_id: split_decision.new_terminal_id,
        active_workspace_id: workspaces.active_id().map(str::to_string),
        terminal_in_active_layout: true,
    });
    apply_terminal_created(&mut tabs, &mut workspaces, created);

    assert_eq!(tabs.active_id(), Some("t-2"));
    let active_before_switch = workspaces.active().expect("active workspace");
    assert_eq!(active_before_switch.focused_terminal, "t-4");
    assert_eq!(
        active_before_switch.layout.all_leaf_ids(),
        vec!["t-1", "t-2", "t-4"]
    );

    let selection = workspace_reducer::reduce_workspace_selection(
        workspace_reducer::WorkspaceSelectionInput {
            workspace_id: "w-2".into(),
            focused_terminal_id: workspaces.get("w-2").map(|ws| ws.focused_terminal.clone()),
        },
    );
    assert!(workspaces.set_active(&selection.workspace_id));
    assert_eq!(
        workspace_reducer::reduce_workspace_switch_read_target(selection.mark_terminal_read_id)
            .as_deref(),
        Some("t-3")
    );

    let delete = workspace_reducer::reduce_delete_workspace(
        workspace_reducer::DeleteWorkspaceInput {
            workspace_count: workspaces.count(),
            terminal_ids: vec!["t-3".into()],
        },
    );
    let deleting_workspace_id = workspaces
        .active_id()
        .expect("workspace should still be active")
        .to_string();
    match delete {
        workspace_reducer::DeleteWorkspaceDecision::RejectedLastWorkspace => {
            panic!("workspace should be deletable with count > 1");
        }
        workspace_reducer::DeleteWorkspaceDecision::Delete { terminal_ids, .. } => {
            for terminal_id in terminal_ids {
                assert!(tabs.close(&terminal_id), "terminal {terminal_id} should close");
            }
            assert!(workspaces.remove(&deleting_workspace_id));
        }
    }

    let post_delete = workspace_reducer::reduce_post_workspace_delete(
        workspaces.active().map(|workspace| workspace.focused_terminal.clone()),
    );
    assert_eq!(workspaces.active_id(), Some("w-1"));
    assert!(!tabs.contains("t-3"));
    assert_eq!(tabs.active_id(), Some("t-2"));
    assert_eq!(post_delete.mark_terminal_read_id.as_deref(), Some("t-4"));
    assert_eq!(post_delete.fetch_grid_terminal_id.as_deref(), Some("t-4"));
}

#[test]
fn close_tab_then_delete_workspace_flow_retargets_focus_and_read_marking() {
    let mut tabs = TabState::new();
    assert!(tabs.open("t-1"));
    assert!(tabs.open("t-2"));
    assert!(tabs.open("t-3"));

    let mut workspaces = WorkspaceCollection::new();
    workspaces.add(workspace(
        "w-1",
        "t-1",
        split("t-1", "t-2", SplitDirection::Horizontal),
    ));
    workspaces.add(workspace("w-2", "t-3", leaf("t-3")));

    let active_workspace = workspaces.active_mut().expect("active workspace");
    let close_decision = layout_reducer::reduce_close_terminal(layout_reducer::CloseTerminalInput {
        layout: active_workspace.layout.clone(),
        focused_terminal_id: active_workspace.focused_terminal.clone(),
        closing_terminal_id: "t-1".into(),
    });
    active_workspace.layout = close_decision.next_layout;
    if let Some(next_focused_terminal_id) = close_decision.next_focused_terminal_id {
        active_workspace.focused_terminal = next_focused_terminal_id;
    }
    assert!(tabs.close("t-1"));
    assert_eq!(active_workspace.layout.all_leaf_ids(), vec!["t-2"]);
    assert_eq!(active_workspace.focused_terminal, "t-2");

    let deleting_workspace_id = workspaces
        .active_id()
        .expect("workspace should still be active")
        .to_string();
    let terminal_ids: Vec<String> = workspaces
        .active()
        .expect("active workspace")
        .layout
        .all_leaf_ids()
        .into_iter()
        .map(str::to_string)
        .collect();

    let delete_decision =
        workspace_reducer::reduce_delete_workspace(workspace_reducer::DeleteWorkspaceInput {
            workspace_count: workspaces.count(),
            terminal_ids,
        });

    match delete_decision {
        workspace_reducer::DeleteWorkspaceDecision::RejectedLastWorkspace => {
            panic!("workspace should be deletable with count > 1");
        }
        workspace_reducer::DeleteWorkspaceDecision::Delete { terminal_ids, .. } => {
            for terminal_id in terminal_ids {
                assert!(tabs.close(&terminal_id), "terminal {terminal_id} should close");
            }
            assert!(workspaces.remove(&deleting_workspace_id));
        }
    }

    let post_delete = workspace_reducer::reduce_post_workspace_delete(
        workspaces.active().map(|workspace| workspace.focused_terminal.clone()),
    );

    assert_eq!(workspaces.active_id(), Some("w-2"));
    assert_eq!(tabs.active_id(), Some("t-3"));
    assert_eq!(post_delete.mark_terminal_read_id.as_deref(), Some("t-3"));
    assert_eq!(post_delete.fetch_grid_terminal_id.as_deref(), Some("t-3"));
}
