/**
 * Recursive layout tree types for split pane management.
 *
 * A layout is either a single terminal (LeafNode) or a binary split
 * (SplitNode) whose children are themselves layout nodes. This allows
 * arbitrary nesting of horizontal and vertical splits.
 */

export interface LeafNode {
  type: 'leaf';
  terminal_id: string;
}

export interface SplitNode {
  type: 'split';
  direction: 'horizontal' | 'vertical';
  ratio: number;
  first: LayoutNode;
  second: LayoutNode;
}

export type LayoutNode = LeafNode | SplitNode;

// ---------------------------------------------------------------------------
// Tree utility functions
// ---------------------------------------------------------------------------

/** Check whether a terminal ID exists anywhere in the tree. */
export function findTerminal(node: LayoutNode, id: string): boolean {
  if (node.type === 'leaf') {
    return node.terminal_id === id;
  }
  return findTerminal(node.first, id) || findTerminal(node.second, id);
}

/** Check whether a terminal exists in the tree. */
export function containsTerminal(node: LayoutNode, terminalId: string): boolean {
  if (node.type === 'leaf') return node.terminal_id === terminalId;
  return containsTerminal(node.first, terminalId) || containsTerminal(node.second, terminalId);
}

/** Collect all terminal IDs from the tree via depth-first traversal. */
export function terminalIds(node: LayoutNode): string[] {
  if (node.type === 'leaf') return [node.terminal_id];
  return [...terminalIds(node.first), ...terminalIds(node.second)];
}

/** Count the number of leaf nodes (terminal panes) in the tree. */
export function countLeaves(node: LayoutNode): number {
  if (node.type === 'leaf') {
    return 1;
  }
  return countLeaves(node.first) + countLeaves(node.second);
}

/** Find the leaf containing `terminalId` and replace it with `replacement`. */
export function replaceLeaf(
  node: LayoutNode,
  terminalId: string,
  replacement: LayoutNode,
): LayoutNode | null {
  if (node.type === 'leaf') {
    return node.terminal_id === terminalId ? replacement : null;
  }
  const firstResult = replaceLeaf(node.first, terminalId, replacement);
  if (firstResult) {
    return { ...node, first: firstResult };
  }
  const secondResult = replaceLeaf(node.second, terminalId, replacement);
  if (secondResult) {
    return { ...node, second: secondResult };
  }
  return null;
}

/**
 * Remove a terminal from the tree.
 * Returns the collapsed tree (the sibling subtree), or null if the terminal
 * was not found.
 */
export function removeLeaf(
  node: LayoutNode,
  terminalId: string,
): { result: LayoutNode | null; found: boolean } {
  if (node.type === 'leaf') {
    if (node.terminal_id === terminalId) {
      return { result: null, found: true };
    }
    return { result: node, found: false };
  }

  // Check first child
  if (node.first.type === 'leaf' && node.first.terminal_id === terminalId) {
    return { result: node.second, found: true };
  }
  if (node.second.type === 'leaf' && node.second.terminal_id === terminalId) {
    return { result: node.first, found: true };
  }

  // Recurse into first
  const firstResult = removeLeaf(node.first, terminalId);
  if (firstResult.found) {
    if (firstResult.result === null) {
      return { result: node.second, found: true };
    }
    return { result: { ...node, first: firstResult.result }, found: true };
  }

  // Recurse into second
  const secondResult = removeLeaf(node.second, terminalId);
  if (secondResult.found) {
    if (secondResult.result === null) {
      return { result: node.first, found: true };
    }
    return { result: { ...node, second: secondResult.result }, found: true };
  }

  return { result: node, found: false };
}

/**
 * Remove a terminal from the tree. When a leaf is removed, its parent split
 * collapses to the sibling node. Returns null if the entire tree is removed
 * (i.e. the root was the removed leaf).
 */
export function removeTerminal(node: LayoutNode, id: string): LayoutNode | null {
  if (node.type === 'leaf') {
    return node.terminal_id === id ? null : node;
  }

  // Check if either child is the target leaf
  if (node.first.type === 'leaf' && node.first.terminal_id === id) {
    return node.second;
  }
  if (node.second.type === 'leaf' && node.second.terminal_id === id) {
    return node.first;
  }

  // Recurse into children
  const newFirst = removeTerminal(node.first, id);
  if (newFirst !== node.first) {
    // Removal happened in the first subtree
    if (newFirst === null) return node.second;
    return { ...node, first: newFirst };
  }

  const newSecond = removeTerminal(node.second, id);
  if (newSecond !== node.second) {
    // Removal happened in the second subtree
    if (newSecond === null) return node.first;
    return { ...node, second: newSecond };
  }

  // ID not found — return unchanged
  return node;
}

/**
 * Get the node at a given path (array of 0=first, 1=second indices).
 * Returns the node or null if the path is invalid.
 */
export function getNodeAtPath(node: LayoutNode, path: number[]): LayoutNode | null {
  if (path.length === 0) return node;
  if (node.type === 'leaf') return null;
  const [head, ...rest] = path;
  if (head === 0) return getNodeAtPath(node.first, rest);
  if (head === 1) return getNodeAtPath(node.second, rest);
  return null;
}

/**
 * Update the ratio of the split node at the given path.
 * Returns the updated tree, or null if the path doesn't point to a split node.
 */
export function updateRatioAtPath(
  node: LayoutNode,
  path: number[],
  ratio: number,
): LayoutNode | null {
  if (path.length === 0) {
    if (node.type !== 'split') return null;
    return { ...node, ratio };
  }
  if (node.type === 'leaf') return null;
  const [head, ...rest] = path;
  if (head === 0) {
    const updated = updateRatioAtPath(node.first, rest, ratio);
    if (!updated) return null;
    return { ...node, first: updated };
  }
  if (head === 1) {
    const updated = updateRatioAtPath(node.second, rest, ratio);
    if (!updated) return null;
    return { ...node, second: updated };
  }
  return null;
}

/**
 * Split a leaf node into two panes. The target leaf (identified by targetId)
 * is replaced by a split node containing the original leaf and a new leaf
 * for newId. The new terminal is placed in the "second" position.
 *
 * Returns the original tree unchanged if targetId is not found.
 */
export function splitAt(
  node: LayoutNode,
  targetId: string,
  newId: string,
  direction: 'horizontal' | 'vertical',
  ratio = 0.5,
): LayoutNode {
  if (node.type === 'leaf') {
    if (node.terminal_id === targetId) {
      return {
        type: 'split',
        direction,
        ratio,
        first: { type: 'leaf', terminal_id: targetId },
        second: { type: 'leaf', terminal_id: newId },
      };
    }
    return node;
  }

  const newFirst = splitAt(node.first, targetId, newId, direction, ratio);
  if (newFirst !== node.first) {
    return { ...node, first: newFirst };
  }

  const newSecond = splitAt(node.second, targetId, newId, direction, ratio);
  if (newSecond !== node.second) {
    return { ...node, second: newSecond };
  }

  return node;
}

/**
 * Swap two terminal IDs in the tree. Both must exist as leaves.
 */
export function swapTerminals(
  node: LayoutNode,
  idA: string,
  idB: string,
): LayoutNode | null {
  if (!containsTerminal(node, idA) || !containsTerminal(node, idB)) return null;
  return mapLeaves(node, (leaf) => {
    if (leaf.terminal_id === idA) return { type: 'leaf', terminal_id: idB };
    if (leaf.terminal_id === idB) return { type: 'leaf', terminal_id: idA };
    return leaf;
  });
}

/** Map over every leaf in the tree. */
function mapLeaves(node: LayoutNode, fn: (leaf: LeafNode) => LeafNode): LayoutNode {
  if (node.type === 'leaf') return fn(node);
  return {
    ...node,
    first: mapLeaves(node.first, fn),
    second: mapLeaves(node.second, fn),
  };
}

/**
 * Update the ratio of a split node that contains the given targetId as a
 * direct child leaf AND matches the given direction. The delta is added to
 * the current ratio, clamped to [0.15, 0.85].
 *
 * This walks the tree and applies the delta to the first matching ancestor
 * split of the target leaf with the specified direction.
 */
export function updateRatio(
  node: LayoutNode,
  targetId: string,
  direction: 'horizontal' | 'vertical',
  delta: number,
): LayoutNode {
  if (node.type === 'leaf') return node;

  // Check if this split directly contains the target and matches direction
  const firstHasTarget = findTerminal(node.first, targetId);
  const secondHasTarget = findTerminal(node.second, targetId);

  if ((firstHasTarget || secondHasTarget) && node.direction === direction) {
    const newRatio = Math.max(0.15, Math.min(0.85, node.ratio + delta));
    return { ...node, ratio: newRatio };
  }

  // Recurse into the child that contains the target
  if (firstHasTarget) {
    const newFirst = updateRatio(node.first, targetId, direction, delta);
    if (newFirst !== node.first) return { ...node, first: newFirst };
  }
  if (secondHasTarget) {
    const newSecond = updateRatio(node.second, targetId, direction, delta);
    if (newSecond !== node.second) return { ...node, second: newSecond };
  }

  return node;
}

/**
 * Convert a legacy flat SplitView into a two-leaf LayoutNode tree.
 */
export function fromLegacySplitView(split: {
  leftTerminalId: string;
  rightTerminalId: string;
  direction: string;
  ratio: number;
}): LayoutNode {
  return {
    type: 'split',
    direction: split.direction === 'vertical' ? 'vertical' : 'horizontal',
    ratio: split.ratio,
    first: { type: 'leaf', terminal_id: split.leftTerminalId },
    second: { type: 'leaf', terminal_id: split.rightTerminalId },
  };
}

/**
 * Walk the tree to find the nearest terminal in the given direction.
 *
 * The algorithm finds the deepest split whose direction matches, where
 * `terminalId` is on one side and we want to move to the other side.
 * `goSecond` = true means move toward the "second" child (right/down),
 * false means toward the "first" child (left/up).
 *
 * Returns the terminal ID of the nearest leaf in that direction, or null.
 */
export function findAdjacentTerminal(
  node: LayoutNode,
  terminalId: string,
  direction: 'horizontal' | 'vertical',
  goSecond: boolean,
): string | null {
  if (node.type === 'leaf') return null;

  const inFirst = containsTerminal(node.first, terminalId);
  const inSecond = containsTerminal(node.second, terminalId);

  if (!inFirst && !inSecond) return null;

  // If this split's direction matches what we're looking for
  if (node.direction === direction) {
    if (inFirst && goSecond) {
      // Moving from first to second — return the nearest leaf in second
      return firstLeaf(node.second);
    }
    if (inSecond && !goSecond) {
      // Moving from second to first — return the last leaf in first
      return lastLeaf(node.first);
    }
  }

  // Recurse into the child that contains the terminal
  if (inFirst) {
    return findAdjacentTerminal(node.first, terminalId, direction, goSecond);
  }
  return findAdjacentTerminal(node.second, terminalId, direction, goSecond);
}

/** Get the first (leftmost/topmost) leaf terminal. */
function firstLeaf(node: LayoutNode): string {
  if (node.type === 'leaf') return node.terminal_id;
  return firstLeaf(node.first);
}

/** Get the last (rightmost/bottommost) leaf terminal. */
function lastLeaf(node: LayoutNode): string {
  if (node.type === 'leaf') return node.terminal_id;
  return lastLeaf(node.second);
}

/**
 * Replace a terminal ID in the tree with a different terminal ID.
 * Used when navigating to a tab outside the current layout — the focused
 * pane is swapped to show the new terminal.
 */
export function replaceTerminal(node: LayoutNode, oldId: string, newId: string): LayoutNode {
  if (node.type === 'leaf') {
    if (node.terminal_id === oldId) return { ...node, terminal_id: newId };
    return node;
  }

  const newFirst = replaceTerminal(node.first, oldId, newId);
  const newSecond = replaceTerminal(node.second, oldId, newId);

  if (newFirst === node.first && newSecond === node.second) return node;
  return { ...node, first: newFirst, second: newSecond };
}
