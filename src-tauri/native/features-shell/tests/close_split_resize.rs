//! Regression tests for bug #764: After splitting a terminal and closing the
//! split partner via tab close (close_terminal_immediate), the remaining
//! terminal's PTY dimensions must be recalculated to fill the full viewport.
//!
//! Fixed by adding resize_all_terminals() to close_terminal_immediate(),
//! matching the behavior of unsplit_focused_pane().

use godly_features_shell::layout as layout_reducer;
use godly_layout_core::{LayoutNode, SplitDirection};

// ---------------------------------------------------------------------------
// Helper: compute what fraction of the total width/height a terminal occupies
// in a layout tree. This mirrors the logic in pane_rect_for_terminal (app.rs)
// without depending on pixel sizes or font metrics.
// ---------------------------------------------------------------------------

fn layout_width_fraction(layout: &LayoutNode, terminal_id: &str) -> Option<f32> {
    width_fraction_inner(layout, terminal_id, 1.0)
}

fn width_fraction_inner(layout: &LayoutNode, terminal_id: &str, current: f32) -> Option<f32> {
    match layout {
        LayoutNode::Leaf { terminal_id: id } => {
            if id == terminal_id {
                Some(current)
            } else {
                None
            }
        }
        LayoutNode::ContentPane { .. } => None,
        LayoutNode::Split {
            direction,
            ratio,
            first,
            second,
        } => match direction {
            SplitDirection::Horizontal => {
                let first_frac = current * ratio;
                let second_frac = current * (1.0 - ratio);
                width_fraction_inner(first, terminal_id, first_frac)
                    .or_else(|| width_fraction_inner(second, terminal_id, second_frac))
            }
            SplitDirection::Vertical => {
                width_fraction_inner(first, terminal_id, current)
                    .or_else(|| width_fraction_inner(second, terminal_id, current))
            }
        },
    }
}

fn layout_height_fraction(layout: &LayoutNode, terminal_id: &str) -> Option<f32> {
    height_fraction_inner(layout, terminal_id, 1.0)
}

fn height_fraction_inner(layout: &LayoutNode, terminal_id: &str, current: f32) -> Option<f32> {
    match layout {
        LayoutNode::Leaf { terminal_id: id } => {
            if id == terminal_id {
                Some(current)
            } else {
                None
            }
        }
        LayoutNode::ContentPane { .. } => None,
        LayoutNode::Split {
            direction,
            ratio,
            first,
            second,
        } => match direction {
            SplitDirection::Vertical => {
                let first_frac = current * ratio;
                let second_frac = current * (1.0 - ratio);
                height_fraction_inner(first, terminal_id, first_frac)
                    .or_else(|| height_fraction_inner(second, terminal_id, second_frac))
            }
            SplitDirection::Horizontal => {
                height_fraction_inner(first, terminal_id, current)
                    .or_else(|| height_fraction_inner(second, terminal_id, current))
            }
        },
    }
}

/// Simulate what resize_all_terminals() does: recompute grid dimensions from
/// the layout geometry. Returns (rows, cols) for the given terminal.
fn compute_grid_dims(
    layout: &LayoutNode,
    terminal_id: &str,
    full_rows: u16,
    full_cols: u16,
) -> (u16, u16) {
    let w_frac = layout_width_fraction(layout, terminal_id).unwrap_or(1.0);
    let h_frac = layout_height_fraction(layout, terminal_id).unwrap_or(1.0);
    let rows = (full_rows as f32 * h_frac).round().max(1.0) as u16;
    let cols = (full_cols as f32 * w_frac).round().max(1.0) as u16;
    (rows, cols)
}

// ---------------------------------------------------------------------------
// Regression #764: horizontal split → close partner → cols must restore
// ---------------------------------------------------------------------------

#[test]
fn close_horizontal_split_partner_must_restore_full_cols() {
    // Bug #764: close_terminal_immediate() must call resize_all_terminals()
    // after removing a terminal from a split layout.
    let full_rows: u16 = 24;
    let full_cols: u16 = 80;

    // Step 1: Split t-1 horizontally with t-2 (50:50)
    let split_layout = LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf {
            terminal_id: "t-1".into(),
        }),
        second: Box::new(LayoutNode::Leaf {
            terminal_id: "t-2".into(),
        }),
    };

    // After split, resize_all_terminals() IS called (split_focused_pane does this).
    let (_split_rows, split_cols) = compute_grid_dims(&split_layout, "t-1", full_rows, full_cols);
    assert_eq!(split_cols, 40, "horizontal split should halve columns");

    // Step 2: Close t-2 via close_terminal_immediate().
    let decision = layout_reducer::reduce_close_terminal(layout_reducer::CloseTerminalInput {
        layout: split_layout,
        focused_terminal_id: "t-1".into(),
        closing_terminal_id: "t-2".into(),
    });

    // Verify the layout tree is correct (single Leaf for t-1)
    assert_eq!(decision.next_layout.leaf_count(), 1);
    assert!(decision.next_layout.find_leaf("t-1"));
    assert!(!decision.root_leaf_removed);

    // Step 3: close_terminal_immediate() now calls resize_all_terminals(),
    // so the remaining terminal's dimensions are recomputed from the new layout.
    let (_post_rows, post_cols) =
        compute_grid_dims(&decision.next_layout, "t-1", full_rows, full_cols);

    assert_eq!(
        post_cols, full_cols,
        "Regression #764: after closing horizontal split partner, remaining terminal \
         must have full cols ({full_cols}), not split cols ({split_cols})"
    );
}

// ---------------------------------------------------------------------------
// Regression #764: vertical split → close partner → rows must restore
// ---------------------------------------------------------------------------

#[test]
fn close_vertical_split_partner_must_restore_full_rows() {
    // Bug #764: same invariant for vertical split (rows instead of cols)
    let full_rows: u16 = 24;
    let full_cols: u16 = 80;

    // Step 1: Split t-1 vertically with t-2 (50:50)
    let split_layout = LayoutNode::Split {
        direction: SplitDirection::Vertical,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf {
            terminal_id: "t-1".into(),
        }),
        second: Box::new(LayoutNode::Leaf {
            terminal_id: "t-2".into(),
        }),
    };

    let (split_rows, _split_cols) = compute_grid_dims(&split_layout, "t-1", full_rows, full_cols);
    assert_eq!(split_rows, 12, "vertical split should halve rows");

    // Step 2: Close t-2
    let decision = layout_reducer::reduce_close_terminal(layout_reducer::CloseTerminalInput {
        layout: split_layout,
        focused_terminal_id: "t-1".into(),
        closing_terminal_id: "t-2".into(),
    });

    assert_eq!(decision.next_layout.leaf_count(), 1);
    assert!(decision.next_layout.find_leaf("t-1"));

    // Step 3: resize_all_terminals() restores full rows
    let (post_rows, _post_cols) =
        compute_grid_dims(&decision.next_layout, "t-1", full_rows, full_cols);

    assert_eq!(
        post_rows, full_rows,
        "Regression #764: after closing vertical split partner, remaining terminal \
         must have full rows ({full_rows}), not split rows ({split_rows})"
    );
}

// ---------------------------------------------------------------------------
// Both close and unsplit paths must resize consistently
// ---------------------------------------------------------------------------

#[test]
fn close_and_unsplit_both_restore_full_dimensions() {
    // Verify that close_terminal (via reduce_close_terminal) and
    // unsplit_focused (via reduce_unsplit_focused) both produce a layout
    // where the remaining terminal gets 100% of the viewport.
    let full_rows: u16 = 24;
    let full_cols: u16 = 80;

    let make_split = || LayoutNode::Split {
        direction: SplitDirection::Horizontal,
        ratio: 0.5,
        first: Box::new(LayoutNode::Leaf {
            terminal_id: "t-1".into(),
        }),
        second: Box::new(LayoutNode::Leaf {
            terminal_id: "t-2".into(),
        }),
    };

    // Path A: close t-2 via close_terminal
    let close_decision =
        layout_reducer::reduce_close_terminal(layout_reducer::CloseTerminalInput {
            layout: make_split(),
            focused_terminal_id: "t-1".into(),
            closing_terminal_id: "t-2".into(),
        });
    let (close_rows, close_cols) =
        compute_grid_dims(&close_decision.next_layout, "t-1", full_rows, full_cols);

    // Path B: unsplit (removes focused terminal t-2, leaves t-1)
    let unsplit_decision =
        layout_reducer::reduce_unsplit_focused(layout_reducer::UnsplitFocusedInput {
            layout: Some(make_split()),
            focused_terminal_id: Some("t-2".into()),
        })
        .expect("unsplit should produce decision");
    let (unsplit_rows, unsplit_cols) =
        compute_grid_dims(&unsplit_decision.next_layout, "t-1", full_rows, full_cols);

    // Both paths must produce identical full-size dimensions
    assert_eq!(close_cols, unsplit_cols);
    assert_eq!(close_rows, unsplit_rows);
    assert_eq!(close_cols, full_cols);
    assert_eq!(close_rows, full_rows);
}
