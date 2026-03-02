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
#[serde(tag = "type", rename_all = "lowercase")]
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
    /// A 2x2 grid layout with independent column/row dividers.
    ///
    /// `col_ratios[0]` is the horizontal split position for the top row,
    /// `col_ratios[1]` is the horizontal split position for the bottom row.
    /// `row_ratios[0]` is the vertical split position for the left column,
    /// `row_ratios[1]` is the vertical split position for the right column.
    /// Children are ordered: TL(0), TR(1), BL(2), BR(3).
    Grid {
        col_ratios: [f64; 2],
        row_ratios: [f64; 2],
        children: [Box<LayoutNode>; 4],
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
            LayoutNode::Grid { children, .. } => {
                children.iter().any(|c| c.find_terminal(terminal_id))
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
            LayoutNode::Grid { children, .. } => {
                for child in children {
                    child.collect_terminal_ids(out);
                }
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
            LayoutNode::Grid { children, .. } => {
                children.iter().map(|c| c.count_leaves()).sum()
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
            LayoutNode::Grid {
                col_ratios,
                row_ratios,
                children,
            } => {
                // Grid children are leaves in practice. Check each child.
                // Find which child (if any) is the target leaf.
                let target_idx = children.iter().position(|c| {
                    matches!(c.as_ref(), LayoutNode::Leaf { terminal_id: id } if id == terminal_id)
                });

                if let Some(idx) = target_idx {
                    // Collapse the grid to a 3-pane SplitNode tree.
                    // Children: TL(0), TR(1), BL(2), BR(3)
                    let [tl, tr, bl, br] = children.clone();
                    let col_r = *col_ratios;
                    let row_r = *row_ratios;

                    let replacement = match idx {
                        // Remove TL: Split(V, rowR[1], TR, Split(H, colR[1], BL, BR))
                        0 => LayoutNode::Split {
                            direction: SplitDirection::Vertical,
                            ratio: row_r[1],
                            first: tr,
                            second: Box::new(LayoutNode::Split {
                                direction: SplitDirection::Horizontal,
                                ratio: col_r[1],
                                first: bl,
                                second: br,
                            }),
                        },
                        // Remove TR: Split(V, rowR[0], TL, Split(H, colR[1], BL, BR))
                        1 => LayoutNode::Split {
                            direction: SplitDirection::Vertical,
                            ratio: row_r[0],
                            first: tl,
                            second: Box::new(LayoutNode::Split {
                                direction: SplitDirection::Horizontal,
                                ratio: col_r[1],
                                first: bl,
                                second: br,
                            }),
                        },
                        // Remove BL: Split(V, rowR[0], Split(H, colR[0], TL, TR), BR)
                        2 => LayoutNode::Split {
                            direction: SplitDirection::Vertical,
                            ratio: row_r[0],
                            first: Box::new(LayoutNode::Split {
                                direction: SplitDirection::Horizontal,
                                ratio: col_r[0],
                                first: tl,
                                second: tr,
                            }),
                            second: br,
                        },
                        // Remove BR: Split(V, rowR[0], Split(H, colR[0], TL, TR), BL)
                        3 => LayoutNode::Split {
                            direction: SplitDirection::Vertical,
                            ratio: row_r[0],
                            first: Box::new(LayoutNode::Split {
                                direction: SplitDirection::Horizontal,
                                ratio: col_r[0],
                                first: tl,
                                second: tr,
                            }),
                            second: bl,
                        },
                        _ => unreachable!(),
                    };

                    let result = replacement.clone();
                    *self = replacement;
                    return Some(result);
                }

                // Target not a direct child leaf — recurse into children
                for child in children.iter_mut() {
                    if child.find_terminal(terminal_id) {
                        return child.remove_terminal(terminal_id);
                    }
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
            LayoutNode::Grid { children, .. } => {
                for child in children.iter_mut() {
                    if child.split_at(terminal_id, new_terminal_id, direction, ratio) {
                        return true;
                    }
                }
                false
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
            LayoutNode::Grid { children, .. } => {
                for child in children.iter_mut() {
                    child.rename_terminal(from, to);
                }
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
            LayoutNode::Grid { children, .. } => {
                // Children: TL(0), TR(1), BL(2), BR(3)
                // Find which child contains the terminal
                let child_idx = children.iter().position(|c| c.find_terminal(terminal_id));
                let child_idx = match child_idx {
                    Some(i) => i,
                    None => return None,
                };

                // First, recurse into the child to handle nested cases
                if let Some(result) =
                    children[child_idx].find_adjacent_inner(terminal_id, direction, go_second)
                {
                    match result {
                        AdjResult::Found(id) => return Some(AdjResult::Found(id)),
                        AdjResult::Propagate => {
                            // The terminal was found, but no adjacent in that child's subtree.
                            // Navigate to the grid neighbor based on direction.
                            let neighbor_idx = match (child_idx, direction, go_second) {
                                // Horizontal (go_second=true = right, false = left)
                                (0, SplitDirection::Horizontal, true) => Some(1),  // TL → TR
                                (2, SplitDirection::Horizontal, true) => Some(3),  // BL → BR
                                (1, SplitDirection::Horizontal, false) => Some(0), // TR → TL
                                (3, SplitDirection::Horizontal, false) => Some(2), // BR → BL
                                // Vertical (go_second=true = down, false = up)
                                (0, SplitDirection::Vertical, true) => Some(2),  // TL → BL
                                (1, SplitDirection::Vertical, true) => Some(3),  // TR → BR
                                (2, SplitDirection::Vertical, false) => Some(0), // BL → TL
                                (3, SplitDirection::Vertical, false) => Some(1), // BR → TR
                                // Edge cases: no neighbor in that direction (propagate up)
                                _ => None,
                            };

                            match neighbor_idx {
                                Some(ni) => {
                                    let target = if go_second {
                                        children[ni].first_leaf()
                                    } else {
                                        children[ni].last_leaf()
                                    };
                                    return Some(AdjResult::Found(target));
                                }
                                None => return Some(AdjResult::Propagate),
                            }
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
            LayoutNode::Grid {
                col_ratios,
                row_ratios,
                children,
            } => {
                // Prune each child: TL(0), TR(1), BL(2), BR(3)
                let pruned: Vec<Option<LayoutNode>> = children
                    .iter()
                    .map(|c| c.prune_stale_terminal_ids(live_ids))
                    .collect();

                let survivors: Vec<(usize, LayoutNode)> = pruned
                    .into_iter()
                    .enumerate()
                    .filter_map(|(i, opt)| opt.map(|n| (i, n)))
                    .collect();

                match survivors.len() {
                    0 => None,
                    1 => Some(survivors.into_iter().next().unwrap().1),
                    2 => {
                        let (i0, n0) = &survivors[0];
                        let (i1, n1) = &survivors[1];
                        // Determine split direction based on positions
                        let dir = if (*i0 == 0 && *i1 == 1) || (*i0 == 2 && *i1 == 3) {
                            // Same row → horizontal split
                            SplitDirection::Horizontal
                        } else {
                            // Same column or diagonal → vertical split
                            SplitDirection::Vertical
                        };
                        let ratio = match dir {
                            SplitDirection::Horizontal => {
                                if *i0 <= 1 { col_ratios[0] } else { col_ratios[1] }
                            }
                            SplitDirection::Vertical => {
                                if *i0 % 2 == 0 { row_ratios[0] } else { row_ratios[1] }
                            }
                        };
                        Some(LayoutNode::Split {
                            direction: dir,
                            ratio,
                            first: Box::new(n0.clone()),
                            second: Box::new(n1.clone()),
                        })
                    }
                    3 => {
                        // Determine which child is missing and collapse accordingly
                        let alive = [
                            survivors.iter().any(|(i, _)| *i == 0),
                            survivors.iter().any(|(i, _)| *i == 1),
                            survivors.iter().any(|(i, _)| *i == 2),
                            survivors.iter().any(|(i, _)| *i == 3),
                        ];
                        let dead_idx = alive.iter().position(|&a| !a).unwrap();

                        let get = |idx: usize| -> LayoutNode {
                            survivors.iter().find(|(i, _)| *i == idx).unwrap().1.clone()
                        };

                        match dead_idx {
                            // TL dead: Split(V, rowR[1], TR, Split(H, colR[1], BL, BR))
                            0 => Some(LayoutNode::Split {
                                direction: SplitDirection::Vertical,
                                ratio: row_ratios[1],
                                first: Box::new(get(1)),
                                second: Box::new(LayoutNode::Split {
                                    direction: SplitDirection::Horizontal,
                                    ratio: col_ratios[1],
                                    first: Box::new(get(2)),
                                    second: Box::new(get(3)),
                                }),
                            }),
                            // TR dead: Split(V, rowR[0], TL, Split(H, colR[1], BL, BR))
                            1 => Some(LayoutNode::Split {
                                direction: SplitDirection::Vertical,
                                ratio: row_ratios[0],
                                first: Box::new(get(0)),
                                second: Box::new(LayoutNode::Split {
                                    direction: SplitDirection::Horizontal,
                                    ratio: col_ratios[1],
                                    first: Box::new(get(2)),
                                    second: Box::new(get(3)),
                                }),
                            }),
                            // BL dead: Split(V, rowR[0], Split(H, colR[0], TL, TR), BR)
                            2 => Some(LayoutNode::Split {
                                direction: SplitDirection::Vertical,
                                ratio: row_ratios[0],
                                first: Box::new(LayoutNode::Split {
                                    direction: SplitDirection::Horizontal,
                                    ratio: col_ratios[0],
                                    first: Box::new(get(0)),
                                    second: Box::new(get(1)),
                                }),
                                second: Box::new(get(3)),
                            }),
                            // BR dead: Split(V, rowR[0], Split(H, colR[0], TL, TR), BL)
                            3 => Some(LayoutNode::Split {
                                direction: SplitDirection::Vertical,
                                ratio: row_ratios[0],
                                first: Box::new(LayoutNode::Split {
                                    direction: SplitDirection::Horizontal,
                                    ratio: col_ratios[0],
                                    first: Box::new(get(0)),
                                    second: Box::new(get(1)),
                                }),
                                second: Box::new(get(2)),
                            }),
                            _ => unreachable!(),
                        }
                    }
                    4 => Some(LayoutNode::Grid {
                        col_ratios: *col_ratios,
                        row_ratios: *row_ratios,
                        children: [
                            Box::new(survivors[0].1.clone()),
                            Box::new(survivors[1].1.clone()),
                            Box::new(survivors[2].1.clone()),
                            Box::new(survivors[3].1.clone()),
                        ],
                    }),
                    _ => unreachable!(),
                }
            }
        }
    }

    /// Get the first (leftmost/topmost) leaf terminal ID.
    fn first_leaf(&self) -> String {
        match self {
            LayoutNode::Leaf { terminal_id } => terminal_id.clone(),
            LayoutNode::Split { first, .. } => first.first_leaf(),
            LayoutNode::Grid { children, .. } => children[0].first_leaf(),
        }
    }

    /// Get the last (rightmost/bottommost) leaf terminal ID.
    fn last_leaf(&self) -> String {
        match self {
            LayoutNode::Leaf { terminal_id } => terminal_id.clone(),
            LayoutNode::Split { second, .. } => second.last_leaf(),
            LayoutNode::Grid { children, .. } => children[3].last_leaf(),
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
            LayoutNode::Grid { children, .. } => {
                // Grid ratios are managed by the frontend's updateGridRatioAtPath.
                // Just recurse into children for nested split updates.
                for child in children.iter_mut() {
                    if child.update_ratio(terminal_id, direction, delta) {
                        return true;
                    }
                }
                false
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
    // Bug #498: TypeScript ↔ Rust serialization format mismatch
    //
    // The TypeScript LayoutNode uses lowercase type tags ("leaf", "split")
    // but the Rust serde expects PascalCase ("Leaf", "Split") because
    // #[serde(tag = "type")] without rename_all uses the variant name as-is.
    //
    // This causes syncLayoutTreeToBackend() invoke to fail silently:
    // the frontend sends { "type": "leaf", ... } but Rust can't deserialize it.
    // ---------------------------------------------------------------

    #[test]
    fn bug498_deserialize_from_javascript_lowercase_leaf() {
        // Bug #498: syncLayoutTreeToBackend sends { type: "leaf" } from TypeScript.
        // The Rust LayoutNode must accept this format for persistence to work.
        let json = r#"{"type":"leaf","terminal_id":"t1"}"#;
        let result: Result<LayoutNode, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "LayoutNode must deserialize from lowercase 'leaf' type tag (sent by JavaScript). \
             Got error: {:?}",
            result.err()
        );
        assert_eq!(result.unwrap(), leaf("t1"));
    }

    #[test]
    fn bug498_deserialize_from_javascript_lowercase_split() {
        // Bug #498: syncLayoutTreeToBackend sends { type: "split" } from TypeScript.
        let json = r#"{
            "type": "split",
            "direction": "horizontal",
            "ratio": 0.5,
            "first": {"type": "leaf", "terminal_id": "t1"},
            "second": {"type": "leaf", "terminal_id": "t2"}
        }"#;
        let result: Result<LayoutNode, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "LayoutNode must deserialize from lowercase 'split' type tag (sent by JavaScript). \
             Got error: {:?}",
            result.err()
        );
        assert_eq!(
            result.unwrap(),
            split(SplitDirection::Horizontal, 0.5, leaf("t1"), leaf("t2"))
        );
    }

    #[test]
    fn bug498_deserialize_nested_from_javascript_format() {
        // Bug #498: nested split trees from JavaScript use lowercase throughout
        let json = r#"{
            "type": "split",
            "direction": "horizontal",
            "ratio": 0.5,
            "first": {"type": "leaf", "terminal_id": "t1"},
            "second": {
                "type": "split",
                "direction": "vertical",
                "ratio": 0.6,
                "first": {"type": "leaf", "terminal_id": "t2"},
                "second": {"type": "leaf", "terminal_id": "t3"}
            }
        }"#;
        let result: Result<LayoutNode, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "LayoutNode must deserialize nested trees with lowercase type tags. \
             Got error: {:?}",
            result.err()
        );
        let expected = split(
            SplitDirection::Horizontal,
            0.5,
            leaf("t1"),
            split(SplitDirection::Vertical, 0.6, leaf("t2"), leaf("t3")),
        );
        assert_eq!(result.unwrap(), expected);
    }

    #[test]
    fn bug498_rust_serialization_uses_format_typescript_can_read() {
        // Bug #498 (restore path): Rust serializes as PascalCase ("Leaf"/"Split")
        // but TypeScript checks node.type === 'leaf' (lowercase).
        // Verify the Rust output format so we can confirm the mismatch.
        let node = leaf("t1");
        let json = serde_json::to_string(&node).unwrap();
        // The format Rust produces must use lowercase to match TypeScript expectations
        assert!(
            json.contains(r#""type":"leaf""#) || json.contains(r#""type": "leaf""#),
            "Rust should serialize LayoutNode::Leaf with lowercase 'leaf' type tag \
             to match TypeScript's node.type === 'leaf' check. \
             Actual serialization: {}",
            json
        );
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
        let json = r#"{"type":"leaf","terminal_id":"t1"}"#;
        let node: LayoutNode = serde_json::from_str(json).unwrap();
        assert_eq!(node, leaf("t1"));
    }

    #[test]
    fn serde_tagged_split() {
        let json = r#"{
            "type": "split",
            "direction": "horizontal",
            "ratio": 0.5,
            "first": {"type": "leaf", "terminal_id": "t1"},
            "second": {"type": "leaf", "terminal_id": "t2"}
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

    // ---------------------------------------------------------------
    // Grid tests
    // ---------------------------------------------------------------

    fn grid(
        col_ratios: [f64; 2],
        row_ratios: [f64; 2],
        tl: LayoutNode,
        tr: LayoutNode,
        bl: LayoutNode,
        br: LayoutNode,
    ) -> LayoutNode {
        LayoutNode::Grid {
            col_ratios,
            row_ratios,
            children: [
                Box::new(tl),
                Box::new(tr),
                Box::new(bl),
                Box::new(br),
            ],
        }
    }

    #[test]
    fn grid_serde_roundtrip() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let json = serde_json::to_string(&g).unwrap();
        let restored: LayoutNode = serde_json::from_str(&json).unwrap();
        assert_eq!(g, restored);
    }

    #[test]
    fn grid_find_terminal() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        assert!(g.find_terminal("t1"));
        assert!(g.find_terminal("t4"));
        assert!(!g.find_terminal("t5"));
    }

    #[test]
    fn grid_terminal_ids() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        assert_eq!(g.terminal_ids(), vec!["t1", "t2", "t3", "t4"]);
    }

    #[test]
    fn grid_count_leaves() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        assert_eq!(g.count_leaves(), 4);
    }

    #[test]
    fn grid_remove_tl_collapses_to_split() {
        let mut g = grid(
            [0.6, 0.4],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let result = g.remove_terminal("t1");
        assert!(result.is_some());
        assert_eq!(g.count_leaves(), 3);
        assert!(!g.find_terminal("t1"));
        assert!(g.find_terminal("t2"));
        assert!(g.find_terminal("t3"));
        assert!(g.find_terminal("t4"));
    }

    #[test]
    fn grid_remove_tr_collapses_to_split() {
        let mut g = grid(
            [0.6, 0.4],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let result = g.remove_terminal("t2");
        assert!(result.is_some());
        assert_eq!(g.count_leaves(), 3);
        assert!(g.find_terminal("t1"));
        assert!(!g.find_terminal("t2"));
        assert!(g.find_terminal("t3"));
        assert!(g.find_terminal("t4"));
    }

    #[test]
    fn grid_remove_bl_collapses_to_split() {
        let mut g = grid(
            [0.6, 0.4],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let result = g.remove_terminal("t3");
        assert!(result.is_some());
        assert_eq!(g.count_leaves(), 3);
        assert!(g.find_terminal("t1"));
        assert!(g.find_terminal("t2"));
        assert!(!g.find_terminal("t3"));
        assert!(g.find_terminal("t4"));
    }

    #[test]
    fn grid_remove_br_collapses_to_split() {
        let mut g = grid(
            [0.6, 0.4],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let result = g.remove_terminal("t4");
        assert!(result.is_some());
        assert_eq!(g.count_leaves(), 3);
        assert!(g.find_terminal("t1"));
        assert!(g.find_terminal("t2"));
        assert!(g.find_terminal("t3"));
        assert!(!g.find_terminal("t4"));
    }

    #[test]
    fn grid_swap_terminals() {
        let mut g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        assert!(g.swap_terminals("t1", "t4"));
        assert_eq!(g.terminal_ids(), vec!["t4", "t2", "t3", "t1"]);
    }

    #[test]
    fn grid_find_adjacent_right() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        // TL right -> TR
        assert_eq!(
            g.find_adjacent("t1", SplitDirection::Horizontal, true),
            Some("t2".to_string())
        );
        // BL right -> BR
        assert_eq!(
            g.find_adjacent("t3", SplitDirection::Horizontal, true),
            Some("t4".to_string())
        );
    }

    #[test]
    fn grid_find_adjacent_left() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        // TR left -> TL
        assert_eq!(
            g.find_adjacent("t2", SplitDirection::Horizontal, false),
            Some("t1".to_string())
        );
        // BR left -> BL
        assert_eq!(
            g.find_adjacent("t4", SplitDirection::Horizontal, false),
            Some("t3".to_string())
        );
    }

    #[test]
    fn grid_find_adjacent_down() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        // TL down -> BL
        assert_eq!(
            g.find_adjacent("t1", SplitDirection::Vertical, true),
            Some("t3".to_string())
        );
        // TR down -> BR
        assert_eq!(
            g.find_adjacent("t2", SplitDirection::Vertical, true),
            Some("t4".to_string())
        );
    }

    #[test]
    fn grid_find_adjacent_up() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        // BL up -> TL
        assert_eq!(
            g.find_adjacent("t3", SplitDirection::Vertical, false),
            Some("t1".to_string())
        );
        // BR up -> TR
        assert_eq!(
            g.find_adjacent("t4", SplitDirection::Vertical, false),
            Some("t2".to_string())
        );
    }

    #[test]
    fn grid_find_adjacent_edge_no_neighbor() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        // TL has no left neighbor
        assert_eq!(
            g.find_adjacent("t1", SplitDirection::Horizontal, false),
            None
        );
        // TL has no up neighbor
        assert_eq!(
            g.find_adjacent("t1", SplitDirection::Vertical, false),
            None
        );
        // BR has no right neighbor
        assert_eq!(
            g.find_adjacent("t4", SplitDirection::Horizontal, true),
            None
        );
        // BR has no down neighbor
        assert_eq!(
            g.find_adjacent("t4", SplitDirection::Vertical, true),
            None
        );
    }

    #[test]
    fn grid_prune_all_live() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let live: HashSet<String> =
            ["t1", "t2", "t3", "t4"].iter().map(|s| s.to_string()).collect();
        let pruned = g.prune_stale_terminal_ids(&live);
        assert_eq!(pruned, Some(g));
    }

    #[test]
    fn grid_prune_one_dead() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let live: HashSet<String> = ["t1", "t2", "t3"].iter().map(|s| s.to_string()).collect();
        let pruned = g.prune_stale_terminal_ids(&live).unwrap();
        assert_eq!(pruned.count_leaves(), 3);
        assert!(pruned.find_terminal("t1"));
        assert!(pruned.find_terminal("t2"));
        assert!(pruned.find_terminal("t3"));
    }

    #[test]
    fn grid_prune_two_dead() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let live: HashSet<String> = ["t1", "t2"].iter().map(|s| s.to_string()).collect();
        let pruned = g.prune_stale_terminal_ids(&live).unwrap();
        assert_eq!(pruned.count_leaves(), 2);
        assert!(pruned.find_terminal("t1"));
        assert!(pruned.find_terminal("t2"));
    }

    #[test]
    fn grid_prune_three_dead() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let live: HashSet<String> = ["t3"].iter().map(|s| s.to_string()).collect();
        let pruned = g.prune_stale_terminal_ids(&live).unwrap();
        assert_eq!(pruned, leaf("t3"));
    }

    #[test]
    fn grid_prune_all_dead() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        let live: HashSet<String> = HashSet::new();
        assert_eq!(g.prune_stale_terminal_ids(&live), None);
    }

    // Backward compat: old persisted JSON with only Leaf/Split still deserializes
    #[test]
    fn old_json_without_grid_still_deserializes() {
        let json = r#"{
            "type": "Split",
            "direction": "horizontal",
            "ratio": 0.5,
            "first": {"type": "Leaf", "terminal_id": "t1"},
            "second": {"type": "Leaf", "terminal_id": "t2"}
        }"#;
        let node: LayoutNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.count_leaves(), 2);
    }

    #[test]
    fn grid_first_and_last_leaf() {
        let g = grid(
            [0.5, 0.5],
            [0.5, 0.5],
            leaf("t1"),
            leaf("t2"),
            leaf("t3"),
            leaf("t4"),
        );
        assert_eq!(g.first_leaf(), "t1");
        assert_eq!(g.last_leaf(), "t4");
    }
}
