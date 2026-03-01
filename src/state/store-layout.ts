import type { Store, SplitView } from './store';
import type { LayoutNode } from './split-types';
import {
  replaceLeaf,
  removeLeaf,
  containsTerminal,
  updateRatioAtPath,
  swapTerminals,
  findAdjacentTerminal,
} from './split-types';

// ---------------------------------------------------------------------------
// Layout tree CRUD
// ---------------------------------------------------------------------------

export function getLayoutTreeImpl(store: Store, workspaceId: string): LayoutNode | null {
  return store.getState().layoutTrees[workspaceId] ?? null;
}

export function setLayoutTreeImpl(store: Store, workspaceId: string, tree: LayoutNode): void {
  store.setState({
    layoutTrees: { ...store.getState().layoutTrees, [workspaceId]: tree },
    splitViews: { ...store.getState().splitViews, ...store.treeToSplitViews(workspaceId, tree) },
  });
  store.enforceSplitAdjacency(workspaceId);
}

/** Clear the active layout tree for a workspace. Does not affect suspended splits. */
export function clearLayoutTreeImpl(store: Store, workspaceId: string): void {
  const state = store.getState();
  const { [workspaceId]: _t, ...restTrees } = state.layoutTrees;
  const { [workspaceId]: _s, ...restSplits } = state.splitViews;
  const { [workspaceId]: _z, ...restZoomed } = state.zoomedPanes;
  store.setState({
    layoutTrees: restTrees,
    splitViews: restSplits,
    zoomedPanes: restZoomed,
  });
}

export function splitTerminalAtImpl(
  store: Store,
  workspaceId: string,
  targetTerminalId: string,
  newTerminalId: string,
  direction: 'horizontal' | 'vertical',
  ratio = 0.5,
): void {
  const tree = store.getState().layoutTrees[workspaceId];
  const newSplit: LayoutNode = {
    type: 'split',
    direction,
    ratio,
    first: { type: 'leaf', terminal_id: targetTerminalId },
    second: { type: 'leaf', terminal_id: newTerminalId },
  };

  if (!tree) {
    // No tree yet — create one
    setLayoutTreeImpl(store, workspaceId, newSplit);
  } else {
    // Replace the target leaf with a split node
    const newTree = replaceLeaf(tree, targetTerminalId, newSplit);
    if (newTree) {
      setLayoutTreeImpl(store, workspaceId, newTree);
    }
  }
}

export function unsplitTerminalImpl(store: Store, workspaceId: string, terminalId?: string): void {
  const tree = store.getState().layoutTrees[workspaceId];
  if (!tree) return;

  if (!terminalId) {
    clearLayoutTreeImpl(store, workspaceId);
    return;
  }

  if (!containsTerminal(tree, terminalId)) return;

  // Clear zoom if needed
  let zoomedPanes = store.getState().zoomedPanes;
  if (zoomedPanes[workspaceId]) {
    const { [workspaceId]: _, ...rest } = zoomedPanes;
    zoomedPanes = rest;
  }

  const { result } = removeLeaf(tree, terminalId);
  if (!result || result.type === 'leaf') {
    // Collapsed to single leaf or empty — clear tree
    clearLayoutTreeImpl(store, workspaceId);
    store.setState({ zoomedPanes });
  } else {
    store.setState({
      layoutTrees: { ...store.getState().layoutTrees, [workspaceId]: result },
      splitViews: { ...store.getState().splitViews, ...store.treeToSplitViews(workspaceId, result) },
      zoomedPanes,
    });
    store.enforceSplitAdjacency(workspaceId);
  }
}

// ---------------------------------------------------------------------------
// Navigation
// ---------------------------------------------------------------------------

export function getFocusedPaneIdImpl(store: Store, workspaceId: string): string | null {
  const tree = store.getState().layoutTrees[workspaceId];
  if (!tree) return null;
  const activeId = store.getState().activeTerminalId;
  if (activeId && containsTerminal(tree, activeId)) return activeId;
  return null;
}

export function getAdjacentPaneImpl(
  store: Store,
  workspaceId: string,
  terminalId: string,
  direction: 'horizontal' | 'vertical',
  goSecond: boolean,
): string | null {
  const tree = store.getState().layoutTrees[workspaceId];
  if (!tree) return null;
  return findAdjacentTerminal(tree, terminalId, direction, goSecond);
}

// ---------------------------------------------------------------------------
// Resize
// ---------------------------------------------------------------------------

export function updateTreeRatioImpl(store: Store, workspaceId: string, path: number[], ratio: number): void {
  const tree = store.getState().layoutTrees[workspaceId];
  if (!tree) return;

  const clamped = Math.max(0.15, Math.min(0.85, ratio));
  const updated = updateRatioAtPath(tree, path, clamped);
  if (updated) {
    store.setState({
      layoutTrees: { ...store.getState().layoutTrees, [workspaceId]: updated },
      splitViews: { ...store.getState().splitViews, ...store.treeToSplitViews(workspaceId, updated) },
    });
  }
}

// ---------------------------------------------------------------------------
// Zoom
// ---------------------------------------------------------------------------

export function setZoomedPaneImpl(store: Store, workspaceId: string, terminalId: string | null): void {
  if (terminalId === null) {
    const { [workspaceId]: _, ...rest } = store.getState().zoomedPanes;
    store.setState({ zoomedPanes: rest });
  } else {
    store.setState({
      zoomedPanes: { ...store.getState().zoomedPanes, [workspaceId]: terminalId },
    });
  }
}

export function getZoomedPaneImpl(store: Store, workspaceId: string): string | null {
  return store.getState().zoomedPanes[workspaceId] ?? null;
}

// ---------------------------------------------------------------------------
// Swap
// ---------------------------------------------------------------------------

export function swapPanesImpl(store: Store, workspaceId: string, idA: string, idB: string): void {
  const tree = store.getState().layoutTrees[workspaceId];
  if (!tree) return;

  const swapped = swapTerminals(tree, idA, idB);
  if (swapped) {
    store.setState({
      layoutTrees: { ...store.getState().layoutTrees, [workspaceId]: swapped },
      splitViews: { ...store.getState().splitViews, ...store.treeToSplitViews(workspaceId, swapped) },
    });
    store.enforceSplitAdjacency(workspaceId);
  }
}

// ---------------------------------------------------------------------------
// Legacy split view operations (backward compatibility wrappers)
// ---------------------------------------------------------------------------

export function setSplitViewImpl(
  store: Store,
  workspaceId: string,
  leftTerminalId: string,
  rightTerminalId: string,
  direction: 'horizontal' | 'vertical',
  ratio = 0.5,
): void {
  const tree: LayoutNode = {
    type: 'split',
    direction,
    ratio,
    first: { type: 'leaf', terminal_id: leftTerminalId },
    second: { type: 'leaf', terminal_id: rightTerminalId },
  };
  setLayoutTreeImpl(store, workspaceId, tree);
}

export function clearSplitViewImpl(store: Store, workspaceId: string): void {
  clearLayoutTreeImpl(store, workspaceId);
}

export function getSplitViewImpl(store: Store, workspaceId: string): SplitView | null {
  const tree = store.getState().layoutTrees[workspaceId];
  if (!tree || tree.type !== 'split') return null;
  if (tree.first.type !== 'leaf' || tree.second.type !== 'leaf') return null;
  return {
    leftTerminalId: tree.first.terminal_id,
    rightTerminalId: tree.second.terminal_id,
    direction: tree.direction,
    ratio: tree.ratio,
  };
}

export function updateSplitRatioImpl(store: Store, workspaceId: string, ratio: number): void {
  const state = store.getState();
  const tree = state.layoutTrees[workspaceId];
  if (!tree || tree.type !== 'split') return;
  store.setState({
    layoutTrees: {
      ...state.layoutTrees,
      [workspaceId]: { ...tree, ratio },
    },
    splitViews: {
      ...state.splitViews,
      [workspaceId]: { ...state.splitViews[workspaceId], ratio },
    },
  });
}
