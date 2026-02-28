use std::collections::HashSet;
use serde::{Deserialize, Serialize};

/// Direction of a split in the layout tree.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// A recursive binary tree representing the pane layout of a workspace.
///
/// Each leaf holds a terminal ID; each internal node splits space between
/// two children with a direction and ratio.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum LayoutNode {
    Leaf {
        terminal_id: String,
    },
    Split {
        direction: SplitDirection,
        /// Fraction of space allocated to `first` (0.0..=1.0).
        ratio: f64,
        first: Box<LayoutNode>,
        second: Box<LayoutNode>,
    },
}

impl LayoutNode {
    /// Check if a terminal exists anywhere in this tree.
    pub fn find_terminal(&self, terminal_id: &str) -> bool {
        match self {
            LayoutNode::Leaf { terminal_id: id } => id == terminal_id,
            LayoutNode::Split { first, second, .. } => {
                first.find_terminal(terminal_id) || second.find_terminal(terminal_id)
            }
        }
    }

    /// Collect all terminal IDs in this tree (depth-first, left-to-right).
    pub fn terminal_ids(&self) -> Vec<String> {
        let mut ids = Vec::new();
        self.collect_terminal_ids(&mut ids);
        ids
    }

    fn collect_terminal_ids(&self, out: &mut Vec<String>) {
        match self {
            LayoutNode::Leaf { terminal_id } => out.push(terminal_id.clone()),
            LayoutNode::Split { first, second, .. } => {
                first.collect_terminal_ids(out);
                second.collect_terminal_ids(out);
            }
        }
    }

    /// Count the number of leaf (terminal) panes.
    pub fn count_leaves(&self) -> usize {
        match self {
            LayoutNode::Leaf { .. } => 1,
            LayoutNode::Split { first, second, .. } => {
                first.count_leaves() + second.count_leaves()
            }
        }
    }

    /// Remove a terminal from the tree by collapsing its parent split.
    ///
    /// Returns `Some(sibling)` when the terminal is found and removed (the
    /// sibling that replaces the split node), or `None` if the terminal was
    /// the only leaf (tree becomes empty) or was not found.
    ///
    /// When the target is found at the top level (self is the matching leaf),
    /// returns `None` — the caller should delete the tree entirely.
    pub fn remove_terminal(&mut self, terminal_id: &str) -> Option<LayoutNode> {
        match self {
            LayoutNode::Leaf { terminal_id: id } => {
                if id == terminal_id {
                    // The root itself is the target — tree becomes empty
                    None
                } else {
                    // Not found
                    None
                }
            }
            LayoutNode::Split {
                first, second, ..
            } => {
                // Check if the target is a direct child
                if let LayoutNode::Leaf { terminal_id: ref id } = **first {
                    if id == terminal_id {
                        // Remove first, replace self with second
                        let sibling = *second.clone();
                        *self = sibling.clone();
                        return Some(sibling);
                    }
                }
                if let LayoutNode::Leaf { terminal_id: ref id } = **second {
                    if id == terminal_id {
                        // Remove second, replace self with first
                        let sibling = *first.clone();
                        *self = sibling.clone();
                        return Some(sibling);
                    }
                }

                // Recurse into children
                if first.find_terminal(terminal_id) {
                    return first.remove_terminal(terminal_id);
                }
                if second.find_terminal(terminal_id) {
                    return second.remove_terminal(terminal_id);
                }

                None
            }
        }
    }

    /// Split the leaf containing `terminal_id`, replacing it with a Split node
    /// that holds both `terminal_id` (first) and `new_terminal_id` (second).
    ///
    /// Returns `false` if `terminal_id` is not found.
    pub fn split_at(
        &mut self,
        terminal_id: &str,
        new_terminal_id: &str,
        direction: SplitDirection,
        ratio: f64,
    ) -> bool {
        match self {
            LayoutNode::Leaf { terminal_id: id } => {
                if id == terminal_id {
                    let old = LayoutNode::Leaf {
                        terminal_id: id.clone(),
                    };
                    let new = LayoutNode::Leaf {
                        terminal_id: new_terminal_id.to_string(),
                    };
                    *self = LayoutNode::Split {
                        direction,
                        ratio,
                        first: Box::new(old),
                        second: Box::new(new),
                    };
                    true
                } else {
                    false
                }
            }
            LayoutNode::Split { first, second, .. } => {
                if first.split_at(terminal_id, new_terminal_id, direction, ratio) {
                    return true;
                }
                second.split_at(terminal_id, new_terminal_id, direction, ratio)
            }
        }
    }

    /// Swap the positions of two terminals in the tree.
    ///
    /// Returns `false` if either terminal is not found.
    pub fn swap_terminals(&mut self, id_a: &str, id_b: &str) -> bool {
        if !self.find_terminal(id_a) || !self.find_terminal(id_b) {
            return false;
        }
        // Perform the swap by renaming id_a -> sentinel -> id_b -> id_a -> sentinel -> id_b
        // Use a two-pass approach: first rename a->sentinel, b->a, then sentinel->b
        self.rename_terminal(id_a, "\x00__swap_sentinel__\x00");
        self.rename_terminal(id_b, id_a);
        self.rename_terminal("\x00__swap_sentinel__\x00", id_b);
        true
    }

    fn rename_terminal(&mut self, from: &str, to: &str) {
        match self {
            LayoutNode::Leaf { terminal_id } => {
                if terminal_id == from {
                    *terminal_id = to.to_string();
                }
            }
            LayoutNode::Split { first, second, .. } => {
                first.rename_terminal(from, to);
                second.rename_terminal(from, to);
            }
        }
    }

    /// Find the nearest terminal in the given direction from the specified terminal.
    ///
    /// `go_second` controls which direction to navigate:
    /// - For `Horizontal` splits: `go_second=true` means "go right", `false` means "go left"
    /// - For `Vertical` splits: `go_second=true` means "go down", `false` means "go up"
    ///
    /// Returns the ID of the first leaf found in the adjacent subtree, or `None`
    /// if there is no adjacent terminal in that direction.
    pub fn find_adjacent(
        &self,
        terminal_id: &str,
        direction: SplitDirection,
        go_second: bool,
    ) -> Option<String> {
        self.find_adjacent_inner(terminal_id, direction, go_second)
            .and_then(|result| match result {
                AdjResult::Found(id) => Some(id),
                AdjResult::Propagate => None,
            })
    }

    fn find_adjacent_inner(
        &self,
        terminal_id: &str,
        direction: SplitDirection,
        go_second: bool,
    ) -> Option<AdjResult> {
        match self {
            LayoutNode::Leaf { terminal_id: id } => {
                if id == terminal_id {
                    Some(AdjResult::Propagate)
                } else {
                    None
                }
            }
            LayoutNode::Split {
                direction: split_dir,
                first,
                second,
                ..
            } => {
                // Try to find the terminal in the first child
                if let Some(result) = first.find_adjacent_inner(terminal_id, direction, go_second) {
                    match result {
                        AdjResult::Found(id) => return Some(AdjResult::Found(id)),
                        AdjResult::Propagate => {
                            if *split_dir == direction && go_second {
                                // Navigate into the second subtree
                                return Some(AdjResult::Found(second.first_leaf()));
                            }
                            // Direction doesn't match or going the wrong way — propagate up
                            return Some(AdjResult::Propagate);
                        }
                    }
                }

                // Try to find the terminal in the second child
                if let Some(result) = second.find_adjacent_inner(terminal_id, direction, go_second)
                {
                    match result {
                        AdjResult::Found(id) => return Some(AdjResult::Found(id)),
                        AdjResult::Propagate => {
                            if *split_dir == direction && !go_second {
                                // Navigate into the first subtree (going backwards)
                                return Some(AdjResult::Found(first.last_leaf()));
                            }
                            return Some(AdjResult::Propagate);
                        }
                    }
                }

                None
            }
        }
    }

    /// Remove all leaves whose terminal ID is not in `live_ids`.
    /// Collapses splits that lose a child. Returns `None` if the entire tree
    /// is pruned (no live terminals remain).
    pub fn prune_stale_terminal_ids(&self, live_ids: &HashSet<String>) -> Option<LayoutNode> {
        match self {
            LayoutNode::Leaf { terminal_id } => {
                if live_ids.contains(terminal_id) {
                    Some(self.clone())
                } else {
                    None
                }
            }
            LayoutNode::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let pruned_first = first.prune_stale_terminal_ids(live_ids);
                let pruned_second = second.prune_stale_terminal_ids(live_ids);
                match (pruned_first, pruned_second) {
                    (Some(f), Some(s)) => Some(LayoutNode::Split {
                        direction: *direction,
                        ratio: *ratio,
                        first: Box::new(f),
                        second: Box::new(s),
                    }),
                    (Some(f), None) => Some(f),
                    (None, Some(s)) => Some(s),
                    (None, None) => None,
                }
            }
        }
    }

    /// Get the first (leftmost/topmost) leaf terminal ID.
    fn first_leaf(&self) -> String {
        match self {
            LayoutNode::Leaf { terminal_id } => terminal_id.clone(),
            LayoutNode::Split { first, .. } => first.first_leaf(),
        }
    }

    /// Get the last (rightmost/bottommost) leaf terminal ID.
    fn last_leaf(&self) -> String {
        match self {
            LayoutNode::Leaf { terminal_id } => terminal_id.clone(),
            LayoutNode::Split { second, .. } => second.last_leaf(),
        }
    }

    /// Adjust the ratio of the nearest ancestor split in the given direction
    /// that contains `terminal_id`.
    ///
    /// `delta` is added to the ratio, clamped to `[0.1, 0.9]`.
    /// Returns `false` if no matching split is found.
    pub fn update_ratio(
        &mut self,
        terminal_id: &str,
        direction: SplitDirection,
        delta: f64,
    ) -> bool {
        match self {
            LayoutNode::Leaf { .. } => false,
            LayoutNode::Split {
                direction: split_dir,
                ratio,
                first,
                second,
            } => {
                if *split_dir == direction {
                    // Check if the target is in either subtree
                    let in_first = first.find_terminal(terminal_id);
                    let in_second = second.find_terminal(terminal_id);
                    if in_first || in_second {
                        // Try to update a deeper matching split first
                        if in_first && first.update_ratio(terminal_id, direction, delta) {
                            return true;
                        }
                        if in_second && second.update_ratio(terminal_id, direction, delta) {
                            return true;
                        }
                        // No deeper match — update this split
                        *ratio = (*ratio + delta).clamp(0.1, 0.9);
                        return true;
                    }
                }
                // Direction doesn't match, but recurse to find a matching child
                if first.update_ratio(terminal_id, direction, delta) {
                    return true;
                }
                second.update_ratio(terminal_id, direction, delta)
            }
        }
    }
}

/// Internal helper for find_adjacent navigation.
enum AdjResult {
    /// Found the adjacent terminal.
    Found(String),
    /// Terminal was found but no adjacent in this subtree — propagate to parent.
    Propagate,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(id: &str) -> LayoutNode {
        LayoutNode::Leaf {
            terminal_id: id.to_string(),
        }
    }

    fn split(dir: SplitDirection, ratio: f64, first: LayoutNode, second: LayoutNode) -> LayoutNode {
        LayoutNode::Split {
            direction: dir,
            ratio,
            first: Box::new(first),
            second: Box::new(second),
        }
    }

    // ---------------------------------------------------------------
    // find_terminal
    // ---------------------------------------------------------------

    #[test]
    fn find_terminal_in_leaf() {
        let node = leaf("t1");
        assert!(node.find_terminal("t1"));
        assert!(!node.find_terminal("t2"));
    }

    #[test]
    fn find_terminal_in_split() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert!(tree.find_terminal("t1"));
        assert!(tree.find_terminal("t2"));
        assert!(!tree.find_terminal("t3"));
    }

    #[test]
    fn find_terminal_nested() {
        let tree = split(
            SplitDirection::Horizontal,
            0.5,
            leaf("t1"),
            split(SplitDirection::Vertical, 0.5, leaf("t2"), leaf("t3")),
        );
        assert!(tree.find_terminal("t1"));
        assert!(tree.find_terminal("t2"));
        assert!(tree.find_terminal("t3"));
        assert!(!tree.find_terminal("t4"));
    }

    // ---------------------------------------------------------------
    // terminal_ids
    // ---------------------------------------------------------------

    #[test]
    fn terminal_ids_single_leaf() {
        let node = leaf("t1");
        assert_eq!(node.terminal_ids(), vec!["t1"]);
    }

    #[test]
    fn terminal_ids_split() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert_eq!(tree.terminal_ids(), vec!["t1", "t2"]);
    }

    #[test]
    fn terminal_ids_nested_depth_first() {
        let tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Vertical, 0.5, leaf("t1"), leaf("t2")),
            leaf("t3"),
        );
        assert_eq!(tree.terminal_ids(), vec!["t1", "t2", "t3"]);
    }

    // ---------------------------------------------------------------
    // count_leaves
    // ---------------------------------------------------------------

    #[test]
    fn count_leaves_single() {
        assert_eq!(leaf("t1").count_leaves(), 1);
    }

    #[test]
    fn count_leaves_split() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert_eq!(tree.count_leaves(), 2);
    }

    #[test]
    fn count_leaves_nested() {
        let tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Vertical, 0.5, leaf("t1"), leaf("t2")),
            split(SplitDirection::Vertical, 0.5, leaf("t3"), leaf("t4")),
        );
        assert_eq!(tree.count_leaves(), 4);
    }

    // ---------------------------------------------------------------
    // split_at
    // ---------------------------------------------------------------

    #[test]
    fn split_leaf_creates_two_pane_tree() {
        let mut tree = leaf("t1");
        let result = tree.split_at("t1", "t2", SplitDirection::Horizontal, 0.5);
        assert!(result);
        assert_eq!(tree.count_leaves(), 2);
        assert!(tree.find_terminal("t1"));
        assert!(tree.find_terminal("t2"));

        match &tree {
            LayoutNode::Split {
                direction, ratio, first, second,
            } => {
                assert_eq!(*direction, SplitDirection::Horizontal);
                assert!((ratio - 0.5).abs() < f64::EPSILON);
                assert_eq!(
                    **first,
                    LayoutNode::Leaf { terminal_id: "t1".to_string() }
                );
                assert_eq!(
                    **second,
                    LayoutNode::Leaf { terminal_id: "t2".to_string() }
                );
            }
            _ => panic!("Expected Split node"),
        }
    }

    #[test]
    fn split_again_creates_three_pane_tree() {
        let mut tree = leaf("t1");
        tree.split_at("t1", "t2", SplitDirection::Horizontal, 0.5);
        let result = tree.split_at("t2", "t3", SplitDirection::Vertical, 0.6);
        assert!(result);
        assert_eq!(tree.count_leaves(), 3);
        assert!(tree.find_terminal("t1"));
        assert!(tree.find_terminal("t2"));
        assert!(tree.find_terminal("t3"));
    }

    #[test]
    fn split_nonexistent_terminal_returns_false() {
        let mut tree = leaf("t1");
        assert!(!tree.split_at("t999", "t2", SplitDirection::Horizontal, 0.5));
        // Tree unchanged
        assert_eq!(tree.count_leaves(), 1);
    }

    #[test]
    fn split_preserves_custom_ratio() {
        let mut tree = leaf("t1");
        tree.split_at("t1", "t2", SplitDirection::Vertical, 0.7);
        match &tree {
            LayoutNode::Split { ratio, direction, .. } => {
                assert!((ratio - 0.7).abs() < f64::EPSILON);
                assert_eq!(*direction, SplitDirection::Vertical);
            }
            _ => panic!("Expected Split"),
        }
    }

    // ---------------------------------------------------------------
    // remove_terminal
    // ---------------------------------------------------------------

    #[test]
    fn remove_from_two_pane_returns_sibling() {
        let mut tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        let result = tree.remove_terminal("t1");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), leaf("t2"));
        // Tree is now just the remaining leaf
        assert_eq!(tree, leaf("t2"));
    }

    #[test]
    fn remove_second_from_two_pane_returns_first() {
        let mut tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        let result = tree.remove_terminal("t2");
        assert!(result.is_some());
        assert_eq!(result.unwrap(), leaf("t1"));
        assert_eq!(tree, leaf("t1"));
    }

    #[test]
    fn remove_from_three_pane_collapses_correctly() {
        // Tree: H(t1, V(t2, t3))
        let mut tree = split(
            SplitDirection::Horizontal,
            0.5,
            leaf("t1"),
            split(SplitDirection::Vertical, 0.5, leaf("t2"), leaf("t3")),
        );
        let result = tree.remove_terminal("t2");
        assert!(result.is_some());
        // The V(t2, t3) split collapses to just t3
        assert_eq!(tree.count_leaves(), 2);
        assert!(tree.find_terminal("t1"));
        assert!(tree.find_terminal("t3"));
        assert!(!tree.find_terminal("t2"));
    }

    #[test]
    fn remove_last_terminal_returns_none() {
        let mut tree = leaf("t1");
        let result = tree.remove_terminal("t1");
        assert!(result.is_none());
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        let result = tree.remove_terminal("t999");
        assert!(result.is_none());
        // Tree unchanged
        assert_eq!(tree.count_leaves(), 2);
    }

    #[test]
    fn remove_deeply_nested() {
        // Tree: H(V(t1, t2), V(t3, t4))
        let mut tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Vertical, 0.5, leaf("t1"), leaf("t2")),
            split(SplitDirection::Vertical, 0.5, leaf("t3"), leaf("t4")),
        );
        tree.remove_terminal("t1");
        // V(t1, t2) collapses to t2 → tree becomes H(t2, V(t3, t4))
        assert_eq!(tree.count_leaves(), 3);
        assert!(!tree.find_terminal("t1"));
        assert!(tree.find_terminal("t2"));
        assert!(tree.find_terminal("t3"));
        assert!(tree.find_terminal("t4"));
    }

    // ---------------------------------------------------------------
    // swap_terminals
    // ---------------------------------------------------------------

    #[test]
    fn swap_two_terminals() {
        let mut tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert!(tree.swap_terminals("t1", "t2"));
        assert_eq!(tree.terminal_ids(), vec!["t2", "t1"]);
    }

    #[test]
    fn swap_across_levels() {
        // Tree: H(t1, V(t2, t3))
        let mut tree = split(
            SplitDirection::Horizontal,
            0.5,
            leaf("t1"),
            split(SplitDirection::Vertical, 0.5, leaf("t2"), leaf("t3")),
        );
        assert!(tree.swap_terminals("t1", "t3"));
        assert_eq!(tree.terminal_ids(), vec!["t3", "t2", "t1"]);
    }

    #[test]
    fn swap_nonexistent_returns_false() {
        let mut tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert!(!tree.swap_terminals("t1", "t999"));
        // Tree unchanged
        assert_eq!(tree.terminal_ids(), vec!["t1", "t2"]);
    }

    #[test]
    fn swap_same_terminal_is_noop() {
        let mut tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert!(tree.swap_terminals("t1", "t1"));
        assert_eq!(tree.terminal_ids(), vec!["t1", "t2"]);
    }

    // ---------------------------------------------------------------
    // find_adjacent
    // ---------------------------------------------------------------

    #[test]
    fn find_adjacent_horizontal_right() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert_eq!(
            tree.find_adjacent("t1", SplitDirection::Horizontal, true),
            Some("t2".to_string())
        );
    }

    #[test]
    fn find_adjacent_horizontal_left() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert_eq!(
            tree.find_adjacent("t2", SplitDirection::Horizontal, false),
            Some("t1".to_string())
        );
    }

    #[test]
    fn find_adjacent_no_neighbor() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        // t1 has no left neighbor
        assert_eq!(
            tree.find_adjacent("t1", SplitDirection::Horizontal, false),
            None
        );
        // t2 has no right neighbor
        assert_eq!(
            tree.find_adjacent("t2", SplitDirection::Horizontal, true),
            None
        );
    }

    #[test]
    fn find_adjacent_wrong_direction() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        // Vertical navigation in a horizontal split — no neighbors
        assert_eq!(
            tree.find_adjacent("t1", SplitDirection::Vertical, true),
            None
        );
    }

    #[test]
    fn find_adjacent_nested() {
        // Tree: H(V(t1, t2), t3)
        let tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Vertical, 0.5, leaf("t1"), leaf("t2")),
            leaf("t3"),
        );
        // t1 right neighbor → t3 (crosses the H split)
        assert_eq!(
            tree.find_adjacent("t1", SplitDirection::Horizontal, true),
            Some("t3".to_string())
        );
        // t2 right neighbor → t3 (crosses the H split)
        assert_eq!(
            tree.find_adjacent("t2", SplitDirection::Horizontal, true),
            Some("t3".to_string())
        );
        // t1 down neighbor → t2 (within V split)
        assert_eq!(
            tree.find_adjacent("t1", SplitDirection::Vertical, true),
            Some("t2".to_string())
        );
        // t3 left neighbor → t1 (first leaf of left subtree)
        assert_eq!(
            tree.find_adjacent("t3", SplitDirection::Horizontal, false),
            Some("t2".to_string()) // last_leaf of first subtree
        );
    }

    #[test]
    fn find_adjacent_nonexistent_terminal() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert_eq!(
            tree.find_adjacent("t999", SplitDirection::Horizontal, true),
            None
        );
    }

    // ---------------------------------------------------------------
    // update_ratio
    // ---------------------------------------------------------------

    #[test]
    fn update_ratio_simple() {
        let mut tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert!(tree.update_ratio("t1", SplitDirection::Horizontal, 0.1));
        match &tree {
            LayoutNode::Split { ratio, .. } => {
                assert!((ratio - 0.6).abs() < f64::EPSILON);
            }
            _ => panic!("Expected Split"),
        }
    }

    #[test]
    fn update_ratio_clamped_high() {
        let mut tree = split(SplitDirection::Horizontal, 0.8, leaf("t1"), leaf("t2"));
        tree.update_ratio("t1", SplitDirection::Horizontal, 0.5);
        match &tree {
            LayoutNode::Split { ratio, .. } => {
                assert!((ratio - 0.9).abs() < f64::EPSILON);
            }
            _ => panic!("Expected Split"),
        }
    }

    #[test]
    fn update_ratio_clamped_low() {
        let mut tree = split(SplitDirection::Horizontal, 0.2, leaf("t1"), leaf("t2"));
        tree.update_ratio("t1", SplitDirection::Horizontal, -0.5);
        match &tree {
            LayoutNode::Split { ratio, .. } => {
                assert!((ratio - 0.1).abs() < f64::EPSILON);
            }
            _ => panic!("Expected Split"),
        }
    }

    #[test]
    fn update_ratio_wrong_direction_returns_false() {
        let mut tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        assert!(!tree.update_ratio("t1", SplitDirection::Vertical, 0.1));
    }

    #[test]
    fn update_ratio_nested_prefers_deepest() {
        // Tree: H(H(t1, t2), t3)
        let mut tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2")),
            leaf("t3"),
        );
        tree.update_ratio("t1", SplitDirection::Horizontal, 0.1);
        // The inner H split should have been updated (deepest matching)
        match &tree {
            LayoutNode::Split { first, ratio, .. } => {
                // Outer ratio unchanged
                assert!((ratio - 0.5).abs() < f64::EPSILON);
                match first.as_ref() {
                    LayoutNode::Split { ratio: inner_ratio, .. } => {
                        assert!((inner_ratio - 0.6).abs() < f64::EPSILON);
                    }
                    _ => panic!("Expected inner Split"),
                }
            }
            _ => panic!("Expected outer Split"),
        }
    }

    // ---------------------------------------------------------------
    // Serialization round-trips
    // ---------------------------------------------------------------

    #[test]
    fn serde_leaf_roundtrip() {
        let node = leaf("t1");
        let json = serde_json::to_string(&node).unwrap();
        let restored: LayoutNode = serde_json::from_str(&json).unwrap();
        assert_eq!(node, restored);
    }

    #[test]
    fn serde_split_roundtrip() {
        let tree = split(SplitDirection::Vertical, 0.7, leaf("t1"), leaf("t2"));
        let json = serde_json::to_string(&tree).unwrap();
        let restored: LayoutNode = serde_json::from_str(&json).unwrap();
        assert_eq!(tree, restored);
    }

    #[test]
    fn serde_nested_roundtrip() {
        let tree = split(
            SplitDirection::Horizontal,
            0.5,
            leaf("t1"),
            split(SplitDirection::Vertical, 0.3, leaf("t2"), leaf("t3")),
        );
        let json = serde_json::to_string(&tree).unwrap();
        let restored: LayoutNode = serde_json::from_str(&json).unwrap();
        assert_eq!(tree, restored);
    }

    #[test]
    fn serde_direction_values() {
        let json_h = serde_json::to_string(&SplitDirection::Horizontal).unwrap();
        assert_eq!(json_h, "\"horizontal\"");
        let json_v = serde_json::to_string(&SplitDirection::Vertical).unwrap();
        assert_eq!(json_v, "\"vertical\"");
    }

    #[test]
    fn serde_tagged_leaf() {
        let json = r#"{"type":"Leaf","terminal_id":"t1"}"#;
        let node: LayoutNode = serde_json::from_str(json).unwrap();
        assert_eq!(node, leaf("t1"));
    }

    #[test]
    fn serde_tagged_split() {
        let json = r#"{
            "type": "Split",
            "direction": "horizontal",
            "ratio": 0.5,
            "first": {"type": "Leaf", "terminal_id": "t1"},
            "second": {"type": "Leaf", "terminal_id": "t2"}
        }"#;
        let node: LayoutNode = serde_json::from_str(json).unwrap();
        assert_eq!(
            node,
            split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"))
        );
    }

    // ---------------------------------------------------------------
    // Edge cases
    // ---------------------------------------------------------------

    #[test]
    fn split_at_deeply_nested_leaf() {
        // Build a chain: H(H(H(t1, t2), t3), t4)
        let mut tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(
                SplitDirection::Horizontal,
                0.5,
                split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2")),
                leaf("t3"),
            ),
            leaf("t4"),
        );
        assert!(tree.split_at("t1", "t5", SplitDirection::Vertical, 0.5));
        assert_eq!(tree.count_leaves(), 5);
        assert!(tree.find_terminal("t5"));
    }

    #[test]
    fn remove_all_except_one() {
        // Start with H(V(t1, t2), t3), remove t1 and t3
        let mut tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Vertical, 0.5, leaf("t1"), leaf("t2")),
            leaf("t3"),
        );
        tree.remove_terminal("t1");
        // Now H(t2, t3)
        assert_eq!(tree.count_leaves(), 2);
        tree.remove_terminal("t3");
        // Now just t2
        assert_eq!(tree.count_leaves(), 1);
        assert_eq!(tree, leaf("t2"));
    }

    #[test]
    fn complex_split_remove_sequence() {
        // Start from single leaf, split multiple times, then remove
        let mut tree = leaf("t1");
        tree.split_at("t1", "t2", SplitDirection::Horizontal, 0.5);
        tree.split_at("t1", "t3", SplitDirection::Vertical, 0.5);
        tree.split_at("t2", "t4", SplitDirection::Vertical, 0.5);
        // Tree: H(V(t1, t3), V(t2, t4))
        assert_eq!(tree.count_leaves(), 4);

        tree.remove_terminal("t3");
        // V(t1, t3) collapses → t1; tree: H(t1, V(t2, t4))
        assert_eq!(tree.count_leaves(), 3);
        assert!(!tree.find_terminal("t3"));

        tree.remove_terminal("t4");
        // V(t2, t4) collapses → t2; tree: H(t1, t2)
        assert_eq!(tree.count_leaves(), 2);

        tree.remove_terminal("t1");
        // H(t1, t2) collapses → t2
        assert_eq!(tree, leaf("t2"));
    }

    // ---------------------------------------------------------------
    // prune_stale_terminal_ids
    // ---------------------------------------------------------------

    #[test]
    fn prune_all_live_returns_same_tree() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        let live: HashSet<String> = ["t1", "t2"].iter().map(|s| s.to_string()).collect();
        let pruned = tree.prune_stale_terminal_ids(&live);
        assert_eq!(pruned, Some(tree));
    }

    #[test]
    fn prune_one_dead_collapses_to_sibling() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        let live: HashSet<String> = ["t1"].iter().map(|s| s.to_string()).collect();
        let pruned = tree.prune_stale_terminal_ids(&live);
        assert_eq!(pruned, Some(leaf("t1")));
    }

    #[test]
    fn prune_all_dead_returns_none() {
        let tree = split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"));
        let live: HashSet<String> = HashSet::new();
        assert_eq!(tree.prune_stale_terminal_ids(&live), None);
    }

    #[test]
    fn prune_single_live_leaf_returns_leaf() {
        let node = leaf("t1");
        let live: HashSet<String> = ["t1"].iter().map(|s| s.to_string()).collect();
        assert_eq!(node.prune_stale_terminal_ids(&live), Some(leaf("t1")));
    }

    #[test]
    fn prune_single_dead_leaf_returns_none() {
        let node = leaf("t1");
        let live: HashSet<String> = HashSet::new();
        assert_eq!(node.prune_stale_terminal_ids(&live), None);
    }

    #[test]
    fn prune_nested_dead_collapses_correctly() {
        // Tree: H(V(t1, t2), t3) — t2 is dead
        let tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Vertical, 0.5, leaf("t1"), leaf("t2")),
            leaf("t3"),
        );
        let live: HashSet<String> = ["t1", "t3"].iter().map(|s| s.to_string()).collect();
        let pruned = tree.prune_stale_terminal_ids(&live).unwrap();
        // V(t1, t2) collapses to t1 → result is H(t1, t3)
        assert_eq!(pruned.count_leaves(), 2);
        assert!(pruned.find_terminal("t1"));
        assert!(pruned.find_terminal("t3"));
        assert!(!pruned.find_terminal("t2"));
    }

    #[test]
    fn prune_entire_subtree_dead() {
        // Tree: H(V(t1, t2), t3) — t1 and t2 both dead
        let tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Vertical, 0.5, leaf("t1"), leaf("t2")),
            leaf("t3"),
        );
        let live: HashSet<String> = ["t3"].iter().map(|s| s.to_string()).collect();
        let pruned = tree.prune_stale_terminal_ids(&live).unwrap();
        // Entire left subtree is dead → result is just t3
        assert_eq!(pruned, leaf("t3"));
    }

    #[test]
    fn prune_preserves_structure_when_all_live() {
        // Tree: H(V(t1, t2), V(t3, t4))
        let tree = split(
            SplitDirection::Horizontal,
            0.5,
            split(SplitDirection::Vertical, 0.5, leaf("t1"), leaf("t2")),
            split(SplitDirection::Vertical, 0.5, leaf("t3"), leaf("t4")),
        );
        let live: HashSet<String> = ["t1", "t2", "t3", "t4"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let pruned = tree.prune_stale_terminal_ids(&live);
        assert_eq!(pruned, Some(tree));
    }
}
