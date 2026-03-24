/// Placement of a new pane relative to the target pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitPlacement {
    /// Place new pane to the left of target.
    Left,
    /// Place new pane to the right of target.
    Right,
    /// Place new pane above target.
    Top,
    /// Place new pane below target.
    Bottom,
}

impl SplitPlacement {
    fn direction(self) -> SplitDirection {
        match self {
            SplitPlacement::Left | SplitPlacement::Right => SplitDirection::Horizontal,
            SplitPlacement::Top | SplitPlacement::Bottom => SplitDirection::Vertical,
        }
    }

    fn new_leaf_is_first(self) -> bool {
        matches!(self, SplitPlacement::Left | SplitPlacement::Top)
    }

    fn default_for_direction(direction: SplitDirection) -> Self {
        match direction {
            SplitDirection::Horizontal => SplitPlacement::Right,
            SplitDirection::Vertical => SplitPlacement::Bottom,
        }
    }
}

/// Direction of a split pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Side-by-side (left | right).
    Horizontal,
    /// Stacked (top / bottom).
    Vertical,
}

/// Direction for spatial pane focus navigation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

impl FocusDirection {
    /// The split axis that this direction moves along.
    pub fn split_direction(self) -> SplitDirection {
        match self {
            FocusDirection::Left | FocusDirection::Right => SplitDirection::Horizontal,
            FocusDirection::Up | FocusDirection::Down => SplitDirection::Vertical,
        }
    }

    /// Whether this direction moves toward the second child of a matching split.
    pub fn moves_to_second(self) -> bool {
        matches!(self, FocusDirection::Right | FocusDirection::Down)
    }
}

/// Content that can be displayed in a non-terminal pane.
#[derive(Debug, Clone, PartialEq)]
pub enum PaneContent {
    Terminal { terminal_id: String },
    FileViewer {
        pane_id: String,
        file_path: String,
        file_type: FileViewerType,
    },
}

/// Type of file being viewed in a file pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileViewerType {
    /// Syntax-highlighted source code
    Code,
    /// Rendered markdown
    Markdown,
    /// Image display (png, jpg, gif, svg, webp)
    Image,
}

/// A binary tree of terminal panes.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutNode {
    /// A single terminal pane.
    Leaf { terminal_id: String },
    /// A non-terminal content pane (file viewer, markdown preview, image).
    ContentPane { content: PaneContent },
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
            LayoutNode::ContentPane { .. } => false,
            LayoutNode::Split { first, second, .. } => first.find_leaf(id) || second.find_leaf(id),
        }
    }

    /// Returns `true` if a content pane with the given id exists anywhere in the tree.
    pub fn find_content_pane(&self, id: &str) -> bool {
        match self {
            LayoutNode::Leaf { .. } => false,
            LayoutNode::ContentPane { pane_id } => pane_id == id,
            LayoutNode::Split { first, second, .. } => {
                first.find_content_pane(id) || second.find_content_pane(id)
            }
        }
    }

    /// Collects all content pane IDs in depth-first order.
    pub fn all_content_pane_ids(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        self.collect_content_pane_ids(&mut ids);
        ids
    }

    fn collect_content_pane_ids<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            LayoutNode::Leaf { .. } => {}
            LayoutNode::ContentPane { pane_id } => out.push(pane_id),
            LayoutNode::Split { first, second, .. } => {
                first.collect_content_pane_ids(out);
                second.collect_content_pane_ids(out);
            }
        }
    }

    /// Removes a content pane from its parent split and promotes the sibling.
    ///
    /// Returns `Some(removed_pane_id)` if found, `None` otherwise.
    pub fn unsplit_content_pane(&mut self, target_id: &str) -> Option<String> {
        match self {
            LayoutNode::Leaf { .. } | LayoutNode::ContentPane { .. } => None,
            LayoutNode::Split { first, second, .. } => {
                if let LayoutNode::ContentPane { pane_id } = first.as_ref() {
                    if pane_id == target_id {
                        let removed = pane_id.clone();
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
                if let LayoutNode::ContentPane { pane_id } = second.as_ref() {
                    if pane_id == target_id {
                        let removed = pane_id.clone();
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
                    .unsplit_content_pane(target_id)
                    .or_else(|| second.unsplit_content_pane(target_id))
            }
        }
    }

    /// Splits a leaf node, inserting an arbitrary `LayoutNode` alongside it.
    ///
    /// Finds the leaf with `target_id` and replaces it with a Split containing
    /// the original leaf and the provided `new_node`. Returns `true` if found.
    pub fn split_leaf_with_node(
        &mut self,
        target_id: &str,
        new_node: LayoutNode,
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
                    second: Box::new(new_node),
                };
                true
            }
            LayoutNode::Leaf { .. } | LayoutNode::ContentPane { .. } => false,
            LayoutNode::Split { first, second, .. } => {
                first.split_leaf_with_node(target_id, new_node.clone(), direction)
                    || second.split_leaf_with_node(target_id, new_node, direction)
            }
        }
    }

    /// Counts the total number of leaf nodes in the tree.
    pub fn leaf_count(&self) -> usize {
        match self {
            LayoutNode::Leaf { .. } => 1,
            LayoutNode::ContentPane { .. } => 0,
            LayoutNode::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    /// Collects all leaf terminal IDs in depth-first, first-child-first order.
    /// Content panes are excluded (use `all_content_pane_ids` for those).
    pub fn all_leaf_ids(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        self.collect_leaf_ids(&mut ids);
        ids
    }

    fn collect_leaf_ids<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            LayoutNode::Leaf { terminal_id } => out.push(terminal_id),
            LayoutNode::ContentPane { .. } => {}
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
        self.split_leaf_with_placement(
            target_id,
            new_id,
            SplitPlacement::default_for_direction(direction),
        )
    }

    /// Splits the leaf with `target_id` and inserts `new_id` according to
    /// `placement` (`left`, `right`, `top`, `bottom`) around the target.
    ///
    /// Uses ratio 0.5 (equal split). Returns `true` if the target was found
    /// and split, `false` otherwise.
    pub fn split_leaf_with_placement(
        &mut self,
        target_id: &str,
        new_id: String,
        placement: SplitPlacement,
    ) -> bool {
        match self {
            LayoutNode::Leaf { terminal_id } if terminal_id == target_id => {
                let old = std::mem::replace(
                    self,
                    LayoutNode::Leaf {
                        terminal_id: String::new(),
                    },
                );
                let (first, second) = if placement.new_leaf_is_first() {
                    (
                        LayoutNode::Leaf {
                            terminal_id: new_id,
                        },
                        old,
                    )
                } else {
                    (
                        old,
                        LayoutNode::Leaf {
                            terminal_id: new_id,
                        },
                    )
                };
                *self = LayoutNode::Split {
                    direction: placement.direction(),
                    ratio: 0.5,
                    first: Box::new(first),
                    second: Box::new(second),
                };
                true
            }
            LayoutNode::Leaf { .. } | LayoutNode::ContentPane { .. } => false,
            LayoutNode::Split { first, second, .. } => {
                first.split_leaf_with_placement(target_id, new_id.clone(), placement)
                    || second.split_leaf_with_placement(target_id, new_id, placement)
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
            LayoutNode::Leaf { .. } | LayoutNode::ContentPane { .. } => None,
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

    /// Returns the leftmost/topmost leaf ID (first in depth-first order).
    pub fn first_leaf_id(&self) -> Option<&str> {
        match self {
            LayoutNode::Leaf { terminal_id } => Some(terminal_id),
            LayoutNode::ContentPane { .. } => None,
            LayoutNode::Split { first, second, .. } => {
                first.first_leaf_id().or_else(|| second.first_leaf_id())
            }
        }
    }

    /// Returns the rightmost/bottommost leaf ID (last in depth-first order).
    pub fn last_leaf_id(&self) -> Option<&str> {
        match self {
            LayoutNode::Leaf { terminal_id } => Some(terminal_id),
            LayoutNode::ContentPane { .. } => None,
            LayoutNode::Split { first, second, .. } => {
                second.last_leaf_id().or_else(|| first.last_leaf_id())
            }
        }
    }

    /// Returns the spatial neighbor of `current_id` in the given direction,
    /// or `None` if there is no neighbor in that direction.
    pub fn neighbor_in_direction(&self, current_id: &str, direction: FocusDirection) -> Option<&str> {
        match self {
            LayoutNode::Leaf { .. } | LayoutNode::ContentPane { .. } => None,
            LayoutNode::Split {
                direction: split_dir,
                first,
                second,
                ..
            } => {
                if *split_dir == direction.split_direction() {
                    if direction.moves_to_second() && first.find_leaf(current_id) {
                        return second.first_leaf_id();
                    }
                    if !direction.moves_to_second() && second.find_leaf(current_id) {
                        return first.last_leaf_id();
                    }
                }
                first
                    .neighbor_in_direction(current_id, direction)
                    .or_else(|| second.neighbor_in_direction(current_id, direction))
            }
        }
    }

    // ---- Content pane methods ----

    /// Returns `true` if a content pane with the given pane_id exists.
    pub fn find_content_pane(&self, pane_id: &str) -> bool {
        match self {
            LayoutNode::Leaf { .. } => false,
            LayoutNode::ContentPane {
                content: PaneContent::FileViewer { pane_id: id, .. },
            } => id == pane_id,
            LayoutNode::ContentPane { .. } => false,
            LayoutNode::Split { first, second, .. } => {
                first.find_content_pane(pane_id) || second.find_content_pane(pane_id)
            }
        }
    }

    /// Collects all content pane IDs in depth-first order.
    pub fn all_content_pane_ids(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        self.collect_content_pane_ids(&mut ids);
        ids
    }

    fn collect_content_pane_ids<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            LayoutNode::Leaf { .. } => {}
            LayoutNode::ContentPane {
                content: PaneContent::FileViewer { pane_id, .. },
            } => out.push(pane_id),
            LayoutNode::ContentPane { .. } => {}
            LayoutNode::Split { first, second, .. } => {
                first.collect_content_pane_ids(out);
                second.collect_content_pane_ids(out);
            }
        }
    }

    /// Removes the content pane with `pane_id` from its parent split and
    /// promotes the sibling. Returns Some(pane_id) if found, None otherwise.
    pub fn unsplit_content_pane(&mut self, pane_id: &str) -> Option<String> {
        match self {
            LayoutNode::Leaf { .. } | LayoutNode::ContentPane { .. } => None,
            LayoutNode::Split { first, second, .. } => {
                if let LayoutNode::ContentPane {
                    content: PaneContent::FileViewer { pane_id: id, .. },
                } = first.as_ref()
                {
                    if id == pane_id {
                        let removed = id.clone();
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
                if let LayoutNode::ContentPane {
                    content: PaneContent::FileViewer { pane_id: id, .. },
                } = second.as_ref()
                {
                    if id == pane_id {
                        let removed = id.clone();
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
                    .unsplit_content_pane(pane_id)
                    .or_else(|| second.unsplit_content_pane(pane_id))
            }
        }
    }

    /// Splits the leaf `target_id` and inserts `new_node` as the second child.
    /// This allows inserting a ContentPane beside a terminal.
    pub fn split_leaf_with_node(
        &mut self,
        target_id: &str,
        new_node: LayoutNode,
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
                    second: Box::new(new_node),
                };
                true
            }
            LayoutNode::Leaf { .. } | LayoutNode::ContentPane { .. } => false,
            LayoutNode::Split { first, second, .. } => {
                first.split_leaf_with_node(target_id, new_node.clone(), direction)
                    || second.split_leaf_with_node(target_id, new_node, direction)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_split_shape(
        node: &LayoutNode,
        direction: SplitDirection,
        first_id: &str,
        second_id: &str,
    ) {
        match node {
            LayoutNode::Split {
                direction: actual_direction,
                first,
                second,
                ..
            } => {
                assert_eq!(*actual_direction, direction);
                assert_eq!(first.all_leaf_ids(), vec![first_id]);
                assert_eq!(second.all_leaf_ids(), vec![second_id]);
            }
            LayoutNode::Leaf { .. } => panic!("expected split layout"),
            LayoutNode::ContentPane { .. } => panic!("expected split layout"),
        }
    }

    fn make_content_pane(pane_id: &str, path: &str) -> LayoutNode {
        LayoutNode::ContentPane {
            content: PaneContent::FileViewer {
                pane_id: pane_id.into(),
                file_path: path.into(),
                file_type: FileViewerType::Code,
            },
        }
    }

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
    fn split_leaf_with_left_places_new_leaf_first() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };

        assert!(node.split_leaf_with_placement("t1", "t2".into(), SplitPlacement::Left));
        assert_split_shape(&node, SplitDirection::Horizontal, "t2", "t1");
    }

    #[test]
    fn split_leaf_with_right_places_new_leaf_second() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };

        assert!(node.split_leaf_with_placement("t1", "t2".into(), SplitPlacement::Right));
        assert_split_shape(&node, SplitDirection::Horizontal, "t1", "t2");
    }

    #[test]
    fn split_leaf_with_top_places_new_leaf_first() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };

        assert!(node.split_leaf_with_placement("t1", "t2".into(), SplitPlacement::Top));
        assert_split_shape(&node, SplitDirection::Vertical, "t2", "t1");
    }

    #[test]
    fn split_leaf_with_bottom_places_new_leaf_second() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };

        assert!(node.split_leaf_with_placement("t1", "t2".into(), SplitPlacement::Bottom));
        assert_split_shape(&node, SplitDirection::Vertical, "t1", "t2");
    }

    #[test]
    fn split_leaf_with_placement_nonexistent_target_is_noop() {
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
        let before = node.clone();

        assert!(!node.split_leaf_with_placement("missing", "t3".into(), SplitPlacement::Left));
        assert_eq!(node, before);
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

    // --- Content pane tests ---

    #[test]
    fn test_content_pane_not_found_by_find_leaf() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(make_content_pane("cp1", "main.rs")),
        };
        assert!(node.find_leaf("t1"));
        assert!(!node.find_leaf("cp1"));
    }

    #[test]
    fn test_content_pane_not_counted_in_leaf_count() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(make_content_pane("cp1", "main.rs")),
        };
        assert_eq!(node.leaf_count(), 1);
    }

    #[test]
    fn test_find_content_pane() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(make_content_pane("cp1", "main.rs")),
        };
        assert!(node.find_content_pane("cp1"));
        assert!(!node.find_content_pane("cp2"));
        assert!(!node.find_content_pane("t1"));
    }

    #[test]
    fn test_all_content_pane_ids() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(LayoutNode::Leaf {
                    terminal_id: "t1".into(),
                }),
                second: Box::new(make_content_pane("cp1", "main.rs")),
            }),
            second: Box::new(make_content_pane("cp2", "lib.rs")),
        };
        assert_eq!(node.all_content_pane_ids(), vec!["cp1", "cp2"]);
        assert!(!node.all_content_pane_ids().contains(&"t1"));
    }

    #[test]
    fn test_split_leaf_with_node() {
        let mut node = LayoutNode::Leaf {
            terminal_id: "t1".into(),
        };
        let content = make_content_pane("cp1", "main.rs");

        assert!(node.split_leaf_with_node("t1", content, SplitDirection::Horizontal));

        match &node {
            LayoutNode::Split {
                direction,
                first,
                second,
                ..
            } => {
                assert_eq!(*direction, SplitDirection::Horizontal);
                assert_eq!(first.all_leaf_ids(), vec!["t1"]);
                assert!(second.all_leaf_ids().is_empty());
                assert_eq!(second.all_content_pane_ids(), vec!["cp1"]);
            }
            _ => panic!("expected split layout"),
        }
    }

    #[test]
    fn test_unsplit_content_pane() {
        let mut node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(make_content_pane("cp1", "main.rs")),
        };

        assert_eq!(node.unsplit_content_pane("cp1"), Some("cp1".into()));
        assert_eq!(node.all_leaf_ids(), vec!["t1"]);
        assert_eq!(node.leaf_count(), 1);
    }

    #[test]
    fn test_unsplit_content_pane_nested() {
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
                second: Box::new(make_content_pane("cp1", "main.rs")),
            }),
        };

        assert_eq!(node.unsplit_content_pane("cp1"), Some("cp1".into()));
        // After unsplit, the inner split promoted t2 as the second child
        assert_eq!(node.all_leaf_ids(), vec!["t1", "t2"]);
        assert_eq!(node.leaf_count(), 2);
    }

    #[test]
    fn test_content_pane_skipped_in_next_leaf_id() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(make_content_pane("cp1", "main.rs")),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".into(),
                }),
            }),
        };

        assert_eq!(node.next_leaf_id("t1"), Some("t2"));
        assert_eq!(node.next_leaf_id("t2"), Some("t1"));
        assert_eq!(node.next_leaf_id("cp1"), None);
    }

    #[test]
    fn test_neighbor_skips_content_pane() {
        let node = LayoutNode::Split {
            direction: SplitDirection::Horizontal,
            ratio: 0.5,
            first: Box::new(LayoutNode::Leaf {
                terminal_id: "t1".into(),
            }),
            second: Box::new(LayoutNode::Split {
                direction: SplitDirection::Vertical,
                ratio: 0.5,
                first: Box::new(make_content_pane("cp1", "main.rs")),
                second: Box::new(LayoutNode::Leaf {
                    terminal_id: "t2".into(),
                }),
            }),
        };

        assert_eq!(
            node.neighbor_in_direction("t1", FocusDirection::Right),
            Some("t2")
        );
    }
}
