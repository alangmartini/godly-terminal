use iced::widget::{column, container, row};
use iced::{Element, Length};

pub use godly_layout_core::{LayoutNode, SplitDirection};

/// Converts a float ratio (0.0..1.0) to integer fill portions for two children.
///
/// The ratio represents the proportion of space given to the first child.
/// Returns `(first_portion, second_portion)` as `u16` values that sum to 100.
fn ratio_to_portions(ratio: f32) -> (u16, u16) {
    let clamped = ratio.clamp(0.01, 0.99);
    let first = (clamped * 100.0).round() as u16;
    let second = 100 - first;
    (first, second)
}

/// Renders a layout tree into an iced `Element`.
///
/// - For `Leaf` nodes: delegates to `render_leaf` with the terminal ID.
/// - For `Split` nodes with `Horizontal`: uses a `row![]` with proportional widths.
/// - For `Split` nodes with `Vertical`: uses a `column![]` with proportional heights.
pub fn view_layout<'a, M: Clone + 'a>(
    node: &LayoutNode,
    render_leaf: &dyn Fn(&str) -> Element<'a, M>,
) -> Element<'a, M> {
    match node {
        LayoutNode::Leaf { terminal_id } => render_leaf(terminal_id),
        LayoutNode::Split {
            direction,
            ratio,
            first,
            second,
        } => {
            let (first_portion, second_portion) = ratio_to_portions(*ratio);
            let first_el = view_layout(first, render_leaf);
            let second_el = view_layout(second, render_leaf);

            match direction {
                SplitDirection::Horizontal => {
                    let first_wrapped = container(first_el)
                        .width(Length::FillPortion(first_portion))
                        .height(Length::Fill);
                    let second_wrapped = container(second_el)
                        .width(Length::FillPortion(second_portion))
                        .height(Length::Fill);
                    row![first_wrapped, second_wrapped]
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                }
                SplitDirection::Vertical => {
                    let first_wrapped = container(first_el)
                        .height(Length::FillPortion(first_portion))
                        .width(Length::Fill);
                    let second_wrapped = container(second_el)
                        .height(Length::FillPortion(second_portion))
                        .width(Length::Fill);
                    column![first_wrapped, second_wrapped]
                        .width(Length::Fill)
                        .height(Length::Fill)
                        .into()
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_leaf() {
        let node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        assert!(node.find_leaf("t1"));
        assert_eq!(node.leaf_count(), 1);
        assert_eq!(node.all_leaf_ids(), vec!["t1"]);
    }

    #[test]
    fn test_split_two_leaves() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t2".into(),
            }),
        };

        assert!(node.find_leaf("t1"));
        assert!(node.find_leaf("t2"));
        assert_eq!(node.leaf_count(), 2);
    }

    #[test]
    fn test_nested_splits() {
        // Structure:
        //   Split(H)
        //   ├── t1
        //   └── Split(V)
        //       ├── t2
        //       └── t3
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.6,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".into(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t3".into(),
                }),
            }),
        };

        assert!(node.find_leaf("t1"));
        assert!(node.find_leaf("t2"));
        assert!(node.find_leaf("t3"));
        assert_eq!(node.leaf_count(), 3);
    }

    #[test]
    fn test_all_leaf_ids_order() {
        // Structure:
        //   Split(H)
        //   ├── Split(V)
        //   │   ├── t1
        //   │   └── t2
        //   └── t3
        //
        // Depth-first, first-child-first: t1, t2, t3
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t1".into(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".into(),
                }),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t3".into(),
            }),
        };

        assert_eq!(node.all_leaf_ids(), vec!["t1", "t2", "t3"]);
    }

    #[test]
    fn test_find_nonexistent() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t2".into(),
            }),
        };

        assert!(!node.find_leaf("nonexistent"));
        assert!(!node.find_leaf(""));
        assert!(!node.find_leaf("t3"));
    }

    #[test]
    fn test_ratio_to_portions_standard() {
        assert_eq!(ratio_to_portions(0.5), (50, 50));
        assert_eq!(ratio_to_portions(0.6), (60, 40));
        assert_eq!(ratio_to_portions(0.3), (30, 70));
    }

    #[test]
    fn test_ratio_to_portions_clamping() {
        // Extreme values get clamped to avoid zero-sized panes.
        let (first, second) = ratio_to_portions(0.0);
        assert!(first >= 1);
        assert!(second >= 1);

        let (first, second) = ratio_to_portions(1.0);
        assert!(first >= 1);
        assert!(second >= 1);
    }

    // --- split_leaf tests ---

    #[test]
    fn test_split_leaf_single() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        assert!(node.split_leaf("t1", "t2".into(), SplitDirection::Horizontal));

        // Should now be a Split with two children
        match &node {
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                assert_eq!(*direction, SplitDirection::Horizontal);
                assert!((ratio - 0.5).abs() < f32::EPSILON);
                match first.as_ref() {
                    LayoutNode::Leaf { terminal_id } => assert_eq!(terminal_id, "t1"),
                    _ => panic!("first child should be a Leaf"),
                }
                match second.as_ref() {
                    LayoutNode::Leaf { terminal_id } => assert_eq!(terminal_id, "t2"),
                    _ => panic!("second child should be a Leaf"),
                }
            }
            _ => panic!("root should be a Split after split_leaf"),
        }
        assert_eq!(node.leaf_count(), 2);
    }

    #[test]
    fn test_split_leaf_nested() {
        // Start: Split(H) [t1, t2]
        // Split t2 vertically with t3
        // Result: Split(H) [t1, Split(V) [t2, t3]]
        let mut node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t2".into(),
            }),
        };

        assert!(node.split_leaf("t2", "t3".into(), SplitDirection::Vertical));
        assert_eq!(node.leaf_count(), 3);
        assert_eq!(node.all_leaf_ids(), vec!["t1", "t2", "t3"]);

        // Verify the nested structure
        match &node {
            LayoutNode::Split { second, .. } => match second.as_ref() {
                LayoutNode::Split { direction, .. } => {
                    assert_eq!(*direction, SplitDirection::Vertical);
                }
                _ => panic!("second child should be a Split after nested split"),
            },
            _ => panic!("root should still be a Split"),
        }
    }

    #[test]
    fn test_split_leaf_nonexistent() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        assert!(!node.split_leaf("nonexistent", "t2".into(), SplitDirection::Horizontal));

        // Node should remain unchanged
        match &node {
            LayoutNode::Leaf { terminal_id } => assert_eq!(terminal_id, "t1"),
            _ => panic!("node should remain a Leaf"),
        }
    }

    // --- unsplit_leaf tests ---

    #[test]
    fn test_unsplit_leaf() {
        // Start: Split(H) [t1, t2]
        // Unsplit t1 -> should promote t2 to root
        let mut node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t2".into(),
            }),
        };

        let removed = node.unsplit_leaf("t1");
        assert_eq!(removed, Some("t1".into()));
        match &node {
            LayoutNode::Leaf { terminal_id } => assert_eq!(terminal_id, "t2"),
            _ => panic!("root should be promoted to a Leaf"),
        }
    }

    #[test]
    fn test_unsplit_leaf_nested() {
        // Structure:
        //   Split(H)
        //   +-- t1
        //   +-- Split(V)
        //       +-- t2
        //       +-- t3
        //
        // Unsplit t2 -> sibling t3 promoted:
        //   Split(H)
        //   +-- t1
        //   +-- t3
        let mut node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".into(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t3".into(),
                }),
            }),
        };

        let removed = node.unsplit_leaf("t2");
        assert_eq!(removed, Some("t2".into()));
        assert_eq!(node.leaf_count(), 2);
        assert_eq!(node.all_leaf_ids(), vec!["t1", "t3"]);
    }

    #[test]
    fn test_unsplit_root_leaf() {
        // Cannot unsplit a single root leaf
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        assert_eq!(node.unsplit_leaf("t1"), None);
    }

    #[test]
    fn test_unsplit_nonexistent() {
        let mut node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t2".into(),
            }),
        };
        assert_eq!(node.unsplit_leaf("nonexistent"), None);
        // Tree should be unchanged
        assert_eq!(node.leaf_count(), 2);
    }

    // --- next_leaf_id tests ---

    #[test]
    fn test_next_leaf_id() {
        // Split(H) [t1, Split(V) [t2, t3]]
        // Order: t1, t2, t3
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".into(),
                }),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t3".into(),
                }),
            }),
        };

        assert_eq!(node.next_leaf_id("t1"), Some("t2"));
        assert_eq!(node.next_leaf_id("t2"), Some("t3"));
    }

    #[test]
    fn test_next_leaf_id_wraps() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t2".into(),
            }),
        };

        // Last leaf wraps to first
        assert_eq!(node.next_leaf_id("t2"), Some("t1"));
    }

    #[test]
    fn test_next_leaf_id_single() {
        let node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        // Single leaf wraps to itself
        assert_eq!(node.next_leaf_id("t1"), Some("t1"));
    }

    #[test]
    fn test_next_leaf_id_nonexistent() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Leaf {
                terminal_id: "t2".into(),
            }),
        };
        assert_eq!(node.next_leaf_id("nonexistent"), None);
    }
}
