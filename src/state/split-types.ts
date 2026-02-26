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

/** Collect all terminal IDs from the tree via depth-first traversal. */
export function terminalIds(node: LayoutNode): string[] {
  if (node.type === 'leaf') return [node.terminal_id];
  return [...terminalIds(node.first), ...terminalIds(node.second)];
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

/** Check whether a terminal exists in the tree. */
export function containsTerminal(node: LayoutNode, terminalId: string): boolean {
  if (node.type === 'leaf') return node.terminal_id === terminalId;
  return containsTerminal(node.first, terminalId) || containsTerminal(node.second, terminalId);
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
