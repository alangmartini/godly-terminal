use godly_layout_core::{LayoutNode, SplitDirection, SplitPlacement};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitFocusedInput {
    pub focused_terminal_id: Option<String>,
    pub new_terminal_id: String,
    pub direction: SplitDirection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitFocusedDecision {
    pub focused_terminal_id: String,
    pub new_terminal_id: String,
    pub direction: SplitDirection,
}

pub fn reduce_split_focused(input: SplitFocusedInput) -> Option<SplitFocusedDecision> {
    let focused_terminal_id = input.focused_terminal_id?;
    Some(SplitFocusedDecision {
        focused_terminal_id,
        new_terminal_id: input.new_terminal_id,
        direction: input.direction,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnsplitFocusedInput {
    pub layout: Option<LayoutNode>,
    pub focused_terminal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UnsplitFocusedDecision {
    pub next_layout: LayoutNode,
    pub removed_terminal_id: String,
    pub next_focused_terminal_id: Option<String>,
}

pub fn reduce_unsplit_focused(input: UnsplitFocusedInput) -> Option<UnsplitFocusedDecision> {
    let mut next_layout = input.layout?;
    let focused_terminal_id = input.focused_terminal_id?;
    let removed_terminal_id = next_layout.unsplit_leaf(&focused_terminal_id)?;
    let next_focused_terminal_id = next_layout
        .all_leaf_ids()
        .first()
        .map(|terminal_id| terminal_id.to_string());

    Some(UnsplitFocusedDecision {
        next_layout,
        removed_terminal_id,
        next_focused_terminal_id,
    })
}

pub fn reduce_cycle_focus(
    layout: Option<&LayoutNode>,
    focused_terminal_id: Option<&str>,
) -> Option<String> {
    let layout = layout?;
    let focused_terminal_id = focused_terminal_id?;
    layout.next_leaf_id(focused_terminal_id).map(str::to_string)
}

#[derive(Debug, Clone, PartialEq)]
pub struct CloseTerminalInput {
    pub layout: LayoutNode,
    pub focused_terminal_id: String,
    pub closing_terminal_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CloseTerminalDecision {
    pub next_layout: LayoutNode,
    pub next_focused_terminal_id: Option<String>,
}

pub fn reduce_close_terminal(input: CloseTerminalInput) -> CloseTerminalDecision {
    let mut next_layout = input.layout;
    let _ = next_layout.unsplit_leaf(&input.closing_terminal_id);

    let next_focused_terminal_id = if input.focused_terminal_id == input.closing_terminal_id {
        Some(
            next_layout
                .all_leaf_ids()
                .first()
                .map(|terminal_id| terminal_id.to_string())
                .unwrap_or_default(),
        )
    } else {
        None
    };

    CloseTerminalDecision {
        next_layout,
        next_focused_terminal_id,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTabIntoSplitZoneInput {
    pub layout: Option<LayoutNode>,
    pub source_terminal_id: Option<String>,
    pub target_terminal_id: Option<String>,
    pub placement: SplitPlacement,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DropTabIntoSplitZoneDecision {
    pub next_layout: LayoutNode,
    pub next_focused_terminal_id: String,
}

pub fn reduce_drop_tab_into_split_zone(
    input: DropTabIntoSplitZoneInput,
) -> Option<DropTabIntoSplitZoneDecision> {
    let mut next_layout = input.layout?;
    let source_terminal_id = input.source_terminal_id?;
    let target_terminal_id = input.target_terminal_id?;

    if source_terminal_id.is_empty() || target_terminal_id.is_empty() {
        return None;
    }
    if source_terminal_id == target_terminal_id {
        return None;
    }
    if matches!(
        &next_layout,
        LayoutNode::Leaf { terminal_id } if terminal_id == &source_terminal_id
    ) {
        return None;
    }
    if !next_layout.find_leaf(&source_terminal_id) || !next_layout.find_leaf(&target_terminal_id) {
        return None;
    }
    if next_layout.unsplit_leaf(&source_terminal_id).is_none() {
        return None;
    }
    if !next_layout.split_leaf_with_placement(
        &target_terminal_id,
        source_terminal_id.clone(),
        input.placement,
    ) {
        return None;
    }

    Some(DropTabIntoSplitZoneDecision {
        next_layout,
        next_focused_terminal_id: source_terminal_id,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        reduce_close_terminal, reduce_cycle_focus, reduce_drop_tab_into_split_zone,
        reduce_split_focused, reduce_unsplit_focused, CloseTerminalInput,
        DropTabIntoSplitZoneInput, SplitFocusedInput, UnsplitFocusedInput,
    };
    use godly_layout_core::{LayoutNode, SplitDirection, SplitPlacement};

    fn split_layout() -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t-1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t-2".into(),
            }),
        }
    }

    fn three_leaf_layout() -> LayoutNode {
        LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t-1".into(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t-2".into(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t-3".into(),
                }),
            }),
        }
    }

    #[test]
    fn split_focused_requires_focused_terminal() {
        let none = reduce_split_focused(SplitFocusedInput {
            focused_terminal_id: None,
            new_terminal_id: "t-3".into(),
            direction: SplitDirection::Vertical,
        });
        assert!(none.is_none());

        let some = reduce_split_focused(SplitFocusedInput {
            focused_terminal_id: Some("t-1".into()),
            new_terminal_id: "t-3".into(),
            direction: SplitDirection::Vertical,
        })
        .expect("split should produce a decision");
        assert_eq!(some.focused_terminal_id, "t-1");
        assert_eq!(some.new_terminal_id, "t-3");
        assert_eq!(some.direction, SplitDirection::Vertical);
    }

    #[test]
    fn unsplit_focused_returns_removed_terminal_and_next_focus() {
        let decision = reduce_unsplit_focused(UnsplitFocusedInput {
            layout: Some(split_layout()),
            focused_terminal_id: Some("t-1".into()),
        })
        .expect("unsplit should produce decision");

        assert_eq!(decision.removed_terminal_id, "t-1");
        assert_eq!(decision.next_focused_terminal_id.as_deref(), Some("t-2"));
        assert_eq!(decision.next_layout.all_leaf_ids(), vec!["t-2"]);
    }

    #[test]
    fn unsplit_focused_is_noop_for_single_root_leaf() {
        let decision = reduce_unsplit_focused(UnsplitFocusedInput {
            layout: Some(LayoutNode::Leaf {
                terminal_id: "t-1".into(),
            }),
            focused_terminal_id: Some("t-1".into()),
        });

        assert!(decision.is_none());
    }

    #[test]
    fn cycle_focus_uses_layout_order_with_wrap() {
        let layout = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t-1".into(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t-2".into(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t-3".into(),
                }),
            }),
        };

        assert_eq!(
            reduce_cycle_focus(Some(&layout), Some("t-1")),
            Some("t-2".into())
        );
        assert_eq!(
            reduce_cycle_focus(Some(&layout), Some("t-3")),
            Some("t-1".into())
        );
        assert_eq!(reduce_cycle_focus(Some(&layout), Some("missing")), None);
    }

    #[test]
    fn close_terminal_reducer_updates_focus_when_focused_terminal_closes() {
        let decision = reduce_close_terminal(CloseTerminalInput {
            layout: split_layout(),
            focused_terminal_id: "t-1".into(),
            closing_terminal_id: "t-1".into(),
        });

        assert_eq!(decision.next_layout.all_leaf_ids(), vec!["t-2"]);
        assert_eq!(decision.next_focused_terminal_id.as_deref(), Some("t-2"));
    }

    #[test]
    fn close_terminal_reducer_keeps_focus_hint_none_when_other_terminal_closes() {
        let decision = reduce_close_terminal(CloseTerminalInput {
            layout: split_layout(),
            focused_terminal_id: "t-1".into(),
            closing_terminal_id: "t-2".into(),
        });

        assert_eq!(decision.next_layout.all_leaf_ids(), vec!["t-1"]);
        assert_eq!(decision.next_focused_terminal_id, None);
    }

    #[test]
    fn drop_tab_into_left_zone_places_source_as_first_child() {
        let decision = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Left,
        })
        .expect("drop should produce decision");

        assert_eq!(decision.next_focused_terminal_id, "t-1");
        assert_eq!(decision.next_layout.all_leaf_ids(), vec!["t-1", "t-2"]);
        match decision.next_layout {
            LayoutNode::Split { direction, .. } => {
                assert_eq!(direction, SplitDirection::Horizontal);
            }
            LayoutNode::Leaf { .. } => panic!("expected split layout"),
        }
    }

    #[test]
    fn drop_tab_into_right_zone_places_source_as_second_child() {
        let decision = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Right,
        })
        .expect("drop should produce decision");

        assert_eq!(decision.next_focused_terminal_id, "t-1");
        assert_eq!(decision.next_layout.all_leaf_ids(), vec!["t-2", "t-1"]);
        match decision.next_layout {
            LayoutNode::Split { direction, .. } => {
                assert_eq!(direction, SplitDirection::Horizontal);
            }
            LayoutNode::Leaf { .. } => panic!("expected split layout"),
        }
    }

    #[test]
    fn drop_tab_into_top_zone_uses_vertical_direction_and_first_child() {
        let decision = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Top,
        })
        .expect("drop should produce decision");

        assert_eq!(decision.next_focused_terminal_id, "t-1");
        assert_eq!(decision.next_layout.all_leaf_ids(), vec!["t-1", "t-2"]);
        match decision.next_layout {
            LayoutNode::Split { direction, .. } => {
                assert_eq!(direction, SplitDirection::Vertical);
            }
            LayoutNode::Leaf { .. } => panic!("expected split layout"),
        }
    }

    #[test]
    fn drop_tab_into_bottom_zone_uses_vertical_direction_and_second_child() {
        let decision = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Bottom,
        })
        .expect("drop should produce decision");

        assert_eq!(decision.next_focused_terminal_id, "t-1");
        assert_eq!(decision.next_layout.all_leaf_ids(), vec!["t-2", "t-1"]);
        match decision.next_layout {
            LayoutNode::Split { direction, .. } => {
                assert_eq!(direction, SplitDirection::Vertical);
            }
            LayoutNode::Leaf { .. } => panic!("expected split layout"),
        }
    }

    #[test]
    fn drop_tab_removes_source_then_reinserts_around_target() {
        let decision = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(three_leaf_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("t-3".into()),
            placement: SplitPlacement::Right,
        })
        .expect("drop should produce decision");

        assert_eq!(decision.next_focused_terminal_id, "t-1");
        assert_eq!(
            decision.next_layout.all_leaf_ids(),
            vec!["t-2", "t-3", "t-1"]
        );
    }

    #[test]
    fn drop_tab_rejects_missing_layout() {
        let decision = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: None,
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Left,
        });

        assert!(decision.is_none());
    }

    #[test]
    fn drop_tab_rejects_missing_or_empty_ids() {
        let missing_source = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: None,
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Left,
        });
        assert!(missing_source.is_none());

        let missing_target = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: None,
            placement: SplitPlacement::Left,
        });
        assert!(missing_target.is_none());

        let empty_source = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some(String::new()),
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Left,
        });
        assert!(empty_source.is_none());

        let empty_target = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some(String::new()),
            placement: SplitPlacement::Left,
        });
        assert!(empty_target.is_none());
    }

    #[test]
    fn drop_tab_rejects_same_source_and_target() {
        let decision = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("t-1".into()),
            placement: SplitPlacement::Right,
        });

        assert!(decision.is_none());
    }

    #[test]
    fn drop_tab_rejects_when_source_or_target_id_is_missing_in_layout() {
        let missing_source = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("missing".into()),
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Left,
        });
        assert!(missing_source.is_none());

        let missing_target = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(split_layout()),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("missing".into()),
            placement: SplitPlacement::Left,
        });
        assert!(missing_target.is_none());
    }

    #[test]
    fn drop_tab_rejects_unsplittable_single_root_source() {
        let decision = reduce_drop_tab_into_split_zone(DropTabIntoSplitZoneInput {
            layout: Some(LayoutNode::Leaf {
                terminal_id: "t-1".into(),
            }),
            source_terminal_id: Some("t-1".into()),
            target_terminal_id: Some("t-2".into()),
            placement: SplitPlacement::Left,
        });

        assert!(decision.is_none());
    }
}
