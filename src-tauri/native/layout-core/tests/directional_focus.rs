use godly_layout_core::{FocusDirection, LayoutNode, SplitDirection};

fn h_split(first: &str, second: &str) -> LayoutNode {
    LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf {
            terminal_id: first.into(),
        }),
        second: Box::new(LayoutNode::Leaf {
            terminal_id: second.into(),
        }),
    }
}

fn v_split(first: &str, second: &str) -> LayoutNode {
    LayoutNode::Split {
        direction: SplitDirection::Vertical,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf {
            terminal_id: first.into(),
        }),
        second: Box::new(LayoutNode::Leaf {
            terminal_id: second.into(),
        }),
    }
}

fn leaf(id: &str) -> LayoutNode {
    LayoutNode::Leaf {
        terminal_id: id.into(),
    }
}

// ---------------------------------------------------------------------------
// Horizontal split: [t-left | t-right]
// ---------------------------------------------------------------------------

#[test]
fn h_split_focus_right_from_left() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        layout.neighbor_in_direction("t-left", FocusDirection::Right),
        Some("t-right")
    );
}

#[test]
fn h_split_focus_left_from_right() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        layout.neighbor_in_direction("t-right", FocusDirection::Left),
        Some("t-left")
    );
}

#[test]
fn h_split_focus_right_from_right_is_none() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        layout.neighbor_in_direction("t-right", FocusDirection::Right),
        None
    );
}

#[test]
fn h_split_focus_left_from_left_is_none() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        layout.neighbor_in_direction("t-left", FocusDirection::Left),
        None
    );
}

#[test]
fn h_split_no_vertical_neighbors() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        layout.neighbor_in_direction("t-left", FocusDirection::Up),
        None
    );
    assert_eq!(
        layout.neighbor_in_direction("t-left", FocusDirection::Down),
        None
    );
    assert_eq!(
        layout.neighbor_in_direction("t-right", FocusDirection::Up),
        None
    );
    assert_eq!(
        layout.neighbor_in_direction("t-right", FocusDirection::Down),
        None
    );
}

// ---------------------------------------------------------------------------
// Vertical split: [t-top / t-bottom]
// ---------------------------------------------------------------------------

#[test]
fn v_split_focus_down_from_top() {
    let layout = v_split("t-top", "t-bottom");
    assert_eq!(
        layout.neighbor_in_direction("t-top", FocusDirection::Down),
        Some("t-bottom")
    );
}

#[test]
fn v_split_focus_up_from_bottom() {
    let layout = v_split("t-top", "t-bottom");
    assert_eq!(
        layout.neighbor_in_direction("t-bottom", FocusDirection::Up),
        Some("t-top")
    );
}

#[test]
fn v_split_no_horizontal_neighbors() {
    let layout = v_split("t-top", "t-bottom");
    assert_eq!(
        layout.neighbor_in_direction("t-top", FocusDirection::Left),
        None
    );
    assert_eq!(
        layout.neighbor_in_direction("t-top", FocusDirection::Right),
        None
    );
}

// ---------------------------------------------------------------------------
// Nested: [t-left | (V) [t-top-right / t-bottom-right]]
// ---------------------------------------------------------------------------

#[test]
fn nested_focus_right_enters_first_leaf_of_subtree() {
    let layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(leaf("t-left")),
        second: Box::new(v_split("t-top-right", "t-bottom-right")),
    };
    assert_eq!(
        layout.neighbor_in_direction("t-left", FocusDirection::Right),
        Some("t-top-right")
    );
}

#[test]
fn nested_focus_left_from_subtree_returns_left_pane() {
    let layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(leaf("t-left")),
        second: Box::new(v_split("t-top-right", "t-bottom-right")),
    };
    assert_eq!(
        layout.neighbor_in_direction("t-top-right", FocusDirection::Left),
        Some("t-left")
    );
    assert_eq!(
        layout.neighbor_in_direction("t-bottom-right", FocusDirection::Left),
        Some("t-left")
    );
}

#[test]
fn nested_focus_down_within_subtree() {
    let layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(leaf("t-left")),
        second: Box::new(v_split("t-top-right", "t-bottom-right")),
    };
    assert_eq!(
        layout.neighbor_in_direction("t-top-right", FocusDirection::Down),
        Some("t-bottom-right")
    );
}

#[test]
fn nested_focus_up_from_left_is_none() {
    let layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(leaf("t-left")),
        second: Box::new(v_split("t-top-right", "t-bottom-right")),
    };
    assert_eq!(
        layout.neighbor_in_direction("t-left", FocusDirection::Up),
        None
    );
}

// ---------------------------------------------------------------------------
// 2x2 grid: (H) [(V) [t-tl / t-bl], (V) [t-tr / t-br]]
//
// Visual:  t-tl | t-tr
//          -----+-----
//          t-bl | t-br
// ---------------------------------------------------------------------------

fn grid_layout() -> LayoutNode {
    LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(v_split("t-tl", "t-bl")),
        second: Box::new(v_split("t-tr", "t-br")),
    }
}

#[test]
fn grid_focus_right_from_top_left() {
    assert_eq!(
        grid_layout().neighbor_in_direction("t-tl", FocusDirection::Right),
        Some("t-tr")
    );
}

#[test]
fn grid_focus_down_from_top_left() {
    assert_eq!(
        grid_layout().neighbor_in_direction("t-tl", FocusDirection::Down),
        Some("t-bl")
    );
}

#[test]
fn grid_focus_left_from_top_right() {
    assert_eq!(
        grid_layout().neighbor_in_direction("t-tr", FocusDirection::Left),
        Some("t-bl")
    );
}

#[test]
fn grid_focus_up_from_bottom_right() {
    assert_eq!(
        grid_layout().neighbor_in_direction("t-br", FocusDirection::Up),
        Some("t-tr")
    );
}

#[test]
fn grid_focus_left_from_bottom_left_is_none() {
    assert_eq!(
        grid_layout().neighbor_in_direction("t-bl", FocusDirection::Left),
        None
    );
}

#[test]
fn grid_focus_down_from_bottom_left_is_none() {
    assert_eq!(
        grid_layout().neighbor_in_direction("t-bl", FocusDirection::Down),
        None
    );
}

// ---------------------------------------------------------------------------
// Single leaf — all directions return None
// ---------------------------------------------------------------------------

#[test]
fn single_leaf_all_directions_none() {
    let layout = leaf("t-only");
    assert_eq!(layout.neighbor_in_direction("t-only", FocusDirection::Left), None);
    assert_eq!(layout.neighbor_in_direction("t-only", FocusDirection::Right), None);
    assert_eq!(layout.neighbor_in_direction("t-only", FocusDirection::Up), None);
    assert_eq!(layout.neighbor_in_direction("t-only", FocusDirection::Down), None);
}

// ---------------------------------------------------------------------------
// Missing ID returns None
// ---------------------------------------------------------------------------

#[test]
fn missing_id_returns_none() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        layout.neighbor_in_direction("missing", FocusDirection::Right),
        None
    );
}
