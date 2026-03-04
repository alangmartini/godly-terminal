/// Direction of a split pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Side-by-side (left | right).
    Horizontal,
    /// Stacked (top / bottom).
    Vertical,
}

/// A binary tree of terminal panes.
#[derive(Debug, Clone, PartialEq)]
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
            LayoutNode::Split { first, second, .. } => first.find_leaf(id) || second.find_leaf(id),
        }
    }

    /// Counts the total number of leaf nodes in the tree.
    pub fn leaf_count(&self) -> usize {
        match self {
            LayoutNode::Leaf { .. } => 1,
            LayoutNode::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
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

    /// Splits the leaf with `target_id` into a `Split` node containing
    /// the original leaf as `first` and a new leaf (`new_id`) as `second`.
    ///
    /// Uses ratio 0.5 (equal split). Returns `true` if the target was found
    /// and split, `false` otherwise.
    pub fn split_leaf(
        &mut self,
        target_id: &str,
        new_id: String,
        direction: SplitDirection,
    ) -> bool {
        match self {
            LayoutNode::Leaf { terminal_id } if terminal_id == target_id => {
                let old = std::mem::replace(
                    self,
                    LayoutNode::Leaf {
                        terminal_id: String::new(),
                    },
                );
                *self = LayoutNode::Split {
                    direction,
                    ratio: 0.5,
                    first: Box::new(old),
                    second: Box::new(LayoutNode::Leaf {
                        terminal_id: new_id,
                    }),
                };
                true
            }
            LayoutNode::Leaf { .. } => false,
            LayoutNode::Split { first, second, .. } => {
                first.split_leaf(target_id, new_id.clone(), direction)
                    || second.split_leaf(target_id, new_id, direction)
            }
        }
    }

    /// Removes the leaf with `target_id` from its parent split and promotes
    /// the sibling to take the parent's place.
    ///
    /// Returns `Some(removed_id)` if found, `None` otherwise.
    /// Cannot unsplit the root leaf (if the entire tree is a single leaf, returns `None`).
    pub fn unsplit_leaf(&mut self, target_id: &str) -> Option<String> {
        match self {
            LayoutNode::Leaf { .. } => None,
            LayoutNode::Split { first, second, .. } => {
                if let LayoutNode::Leaf { terminal_id } = first.as_ref() {
                    if terminal_id == target_id {
                        let removed = terminal_id.clone();
                        let sibling = std::mem::replace(
                            second.as_mut(),
                            LayoutNode::Leaf {
                                terminal_id: String::new(),
                            },
                        );
                        *self = sibling;
                        return Some(removed);
                    }
                }
                if let LayoutNode::Leaf { terminal_id } = second.as_ref() {
                    if terminal_id == target_id {
                        let removed = terminal_id.clone();
                        let sibling = std::mem::replace(
                            first.as_mut(),
                            LayoutNode::Leaf {
                                terminal_id: String::new(),
                            },
                        );
                        *self = sibling;
                        return Some(removed);
                    }
                }
                first
                    .unsplit_leaf(target_id)
                    .or_else(|| second.unsplit_leaf(target_id))
            }
        }
    }

    /// Returns the next leaf ID in depth-first order after `current_id`.
    ///
    /// Wraps around from the last leaf to the first. Returns `None` if
    /// `current_id` is not found in the tree.
    pub fn next_leaf_id(&self, current_id: &str) -> Option<&str> {
        let ids = self.all_leaf_ids();
        let pos = ids.iter().position(|&id| id == current_id)?;
        let next_pos = (pos + 1) % ids.len();
        Some(ids[next_pos])
    }
}

#[cfg(test)]
mod tests {
    use super::{LayoutNode, SplitDirection};

    #[test]
    fn split_and_unsplit_round_trip() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        assert!(node.split_leaf("t1", "t2".into(), SplitDirection::Horizontal));
        assert_eq!(node.leaf_count(), 2);
        assert_eq!(node.all_leaf_ids(), vec!["t1", "t2"]);

        assert_eq!(node.unsplit_leaf("t2"), Some("t2".into()));
        assert_eq!(node.leaf_count(), 1);
        assert_eq!(node.all_leaf_ids(), vec!["t1"]);
    }

    #[test]
    fn split_nonexistent_is_noop() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        assert!(!node.split_leaf("missing", "t2".into(), SplitDirection::Vertical));
        assert_eq!(node.leaf_count(), 1);
    }

    #[test]
    fn next_leaf_wraps_in_depth_first_order() {
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
        assert_eq!(node.next_leaf_id("t3"), Some("t1"));
    }

    #[test]
    fn unsplit_root_leaf_is_none() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        assert_eq!(node.unsplit_leaf("t1"), None);
    }
}
