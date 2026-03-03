use iced::widget::{column, container, row};
use iced::{Element, Length};

/// Direction of a split pane.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SplitDirection {
    /// Side-by-side (left | right).
    Horizontal,
    /// Stacked (top / bottom).
    Vertical,
}

/// A binary tree of terminal panes.
#[derive(Debug, Clone)]
pub enum LayoutNode {
    /// A single terminal pane.
    Leaf { terminal_id: String },
    /// A split containing two sub-layouts.
    Split {
        direction: SplitDirection,
        /// Proportion of space given to the first child (0.0..1.0).
        ratio: f32,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

impl LayoutNode {
    /// Returns `true` if a leaf with the given id exists anywhere in the tree.
    pub fn find_leaf(&self, id: &str) -> bool {
        match self {
            LayoutNode::Leaf { terminal_id } => terminal_id == id,
            LayoutNode::Split { first, second, .. } => {
                first.find_leaf(id) || second.find_leaf(id)
            }
        }
    }

    /// Counts the total number of leaf nodes in the tree.
    pub fn leaf_count(&self) -> usize {
        match self {
            LayoutNode::Leaf { .. } => 1,
            LayoutNode::Split { first, second, .. } => {
                first.leaf_count() + second.leaf_count()
            }
        }
    }

    /// Collects all leaf terminal IDs in depth-first, first-child-first order.
    pub fn all_leaf_ids(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        self.collect_leaf_ids(&mut ids);
        ids
    }

    fn collect_leaf_ids<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            LayoutNode::Leaf { terminal_id } => out.push(terminal_id),
            LayoutNode::Split { first, second, .. } => {
                first.collect_leaf_ids(out);
                second.collect_leaf_ids(out);
            }
        }
    }
}

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
}
