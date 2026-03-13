use godly_features_shell::layout::reduce_directional_focus;
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

// ---------------------------------------------------------------------------
// Basic directional focus via reducer
// ---------------------------------------------------------------------------

#[test]
fn h_split_focus_right() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        reduce_directional_focus(Some(&layout), Some("t-left"), FocusDirection::Right),
        Some("t-right".into())
    );
}

#[test]
fn h_split_focus_left() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        reduce_directional_focus(Some(&layout), Some("t-right"), FocusDirection::Left),
        Some("t-left".into())
    );
}

#[test]
fn v_split_focus_down() {
    let layout = v_split("t-top", "t-bottom");
    assert_eq!(
        reduce_directional_focus(Some(&layout), Some("t-top"), FocusDirection::Down),
        Some("t-bottom".into())
    );
}

#[test]
fn v_split_focus_up() {
    let layout = v_split("t-top", "t-bottom");
    assert_eq!(
        reduce_directional_focus(Some(&layout), Some("t-bottom"), FocusDirection::Up),
        Some("t-top".into())
    );
}

// ---------------------------------------------------------------------------
// Cross-axis returns None
// ---------------------------------------------------------------------------

#[test]
fn v_split_focus_right_returns_none() {
    let layout = v_split("t-top", "t-bottom");
    assert_eq!(
        reduce_directional_focus(Some(&layout), Some("t-top"), FocusDirection::Right),
        None
    );
}

#[test]
fn h_split_focus_down_returns_none() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        reduce_directional_focus(Some(&layout), Some("t-left"), FocusDirection::Down),
        None
    );
}

// ---------------------------------------------------------------------------
// 2x2 grid: directional navigation matches spatial positions
// ---------------------------------------------------------------------------

#[test]
fn grid_focus_right_from_top_left_goes_to_top_right() {
    let layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(v_split("t-tl", "t-bl")),
        second: Box::new(v_split("t-tr", "t-br")),
    };
    assert_eq!(
        reduce_directional_focus(Some(&layout), Some("t-tl"), FocusDirection::Right),
        Some("t-tr".into())
    );
}

#[test]
fn grid_focus_up_from_bottom_right_goes_to_top_right() {
    let layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(v_split("t-tl", "t-bl")),
        second: Box::new(v_split("t-tr", "t-br")),
    };
    assert_eq!(
        reduce_directional_focus(Some(&layout), Some("t-br"), FocusDirection::Up),
        Some("t-tr".into())
    );
}

// ---------------------------------------------------------------------------
// Edge cases: None inputs
// ---------------------------------------------------------------------------

#[test]
fn no_layout_returns_none() {
    assert_eq!(
        reduce_directional_focus(None, Some("t-1"), FocusDirection::Right),
        None
    );
}

#[test]
fn no_focused_returns_none() {
    let layout = h_split("t-left", "t-right");
    assert_eq!(
        reduce_directional_focus(Some(&layout), None, FocusDirection::Right),
        None
    );
}
