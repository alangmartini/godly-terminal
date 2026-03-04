use godly_layout_core::{LayoutNode, SplitDirection};

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

#[cfg(test)]
mod tests {
    use super::{
        reduce_close_terminal, reduce_cycle_focus, reduce_split_focused, reduce_unsplit_focused,
        CloseTerminalInput, SplitFocusedInput, UnsplitFocusedInput,
    };
    use godly_layout_core::{LayoutNode, SplitDirection};

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
}
