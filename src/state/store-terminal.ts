import { invoke } from '@tauri-apps/api/core';
import type { Store, Terminal } from './store';
import {
  containsTerminal,
  removeLeaf,
  terminalIds,
} from './split-types';

// ---------------------------------------------------------------------------
// Terminal operations
// ---------------------------------------------------------------------------

export function addTerminalImpl(
  store: Store,
  terminal: Terminal,
  opts?: { background?: boolean },
): void {
  const state = store.getState();
  const workspaceTerminals = state.terminals.filter(
    t => t.workspaceId === terminal.workspaceId
  );
  const order = workspaceTerminals.length;

  if (opts?.background) {
    store.setState({
      terminals: [...state.terminals, { ...terminal, order }],
    });
  } else {
    store.setLastActiveTerminal(terminal.workspaceId, terminal.id);

    // Clear the layout tree if the new terminal's workspace has an active split,
    // since the new terminal is not in the tree (Bug #391).
    let layoutTrees = state.layoutTrees;
    let splitViews = state.splitViews;
    let zoomedPanes = state.zoomedPanes;
    if (layoutTrees[terminal.workspaceId]) {
      const { [terminal.workspaceId]: _t, ...restTrees } = layoutTrees;
      const { [terminal.workspaceId]: _s, ...restSplits } = splitViews;
      const { [terminal.workspaceId]: _z, ...restZoomed } = zoomedPanes;
      store.setSuspendedLayoutTree(terminal.workspaceId, {
        tree: _t,
        splitView: _s,
        zoomedPane: _z,
      });
      layoutTrees = restTrees;
      splitViews = restSplits;
      zoomedPanes = restZoomed;
    }

    store.setState({
      terminals: [...state.terminals, { ...terminal, order }],
      activeTerminalId: terminal.id,
      layoutTrees,
      splitViews,
      zoomedPanes,
    });
  }
  store.syncSessionPauseState();
}

export function updateTerminalImpl(store: Store, id: string, updates: Partial<Terminal>): void {
  const existing = store.getState().terminals.find(t => t.id === id);
  if (!existing) return;

  // Skip if no values actually changed
  const changed = Object.entries(updates).some(
    ([key, value]) => existing[key as keyof Terminal] !== value
  );
  if (!changed) return;

  store.setState({
    terminals: store.getState().terminals.map(t =>
      t.id === id ? { ...t, ...updates } : t
    ),
  });
}

export function removeTerminalImpl(store: Store, id: string, force = false): void {
  const state = store.getState();
  const terminal = state.terminals.find(t => t.id === id);

  // Pinned tabs cannot be closed unless explicitly forced (unpin first)
  if (terminal?.pinned && !force) return;
  const remainingTerminals = state.terminals.filter(t => t.id !== id);

  let newActiveId = state.activeTerminalId;
  if (state.activeTerminalId === id && terminal) {
    const sameWorkspace = remainingTerminals.filter(
      t => t.workspaceId === terminal.workspaceId
    );
    newActiveId = sameWorkspace[0]?.id ?? null;
  }

  let layoutTrees = state.layoutTrees;
  let splitViews = state.splitViews;
  let zoomedPanes = state.zoomedPanes;

  if (terminal) {
    const wsId = terminal.workspaceId;

    // Invalidate suspended tree if the removed terminal was part of it
    const suspended = store.getSuspendedLayoutTree(wsId);
    if (suspended && containsTerminal(suspended.tree, id)) {
      store.deleteSuspendedLayoutTree(wsId);
    }

    const tree = layoutTrees[wsId];

    if (tree && containsTerminal(tree, id)) {
      // Clear zoom if the zoomed pane is being removed
      if (zoomedPanes[wsId] === id) {
        const { [wsId]: _, ...rest } = zoomedPanes;
        zoomedPanes = rest;
      }

      const { result } = removeLeaf(tree, id);
      if (!result || result.type === 'leaf') {
        // Tree collapsed to a single leaf or empty — clear layout tree
        const { [wsId]: _t, ...restTrees } = layoutTrees;
        const { [wsId]: _s, ...restSplits } = splitViews;
        layoutTrees = restTrees;
        splitViews = restSplits;
        // Set the remaining terminal as active
        if (result && result.type === 'leaf') {
          if (remainingTerminals.some(t => t.id === result.terminal_id)) {
            newActiveId = result.terminal_id;
          }
        }
      } else {
        // Tree still has multiple panes — update it
        layoutTrees = { ...layoutTrees, [wsId]: result };
        splitViews = { ...splitViews, ...store.treeToSplitViews(wsId, result) };
        // Pick a remaining terminal from the tree as active
        const remaining = terminalIds(result);
        if (remaining.length > 0 && (newActiveId === id || !remaining.includes(newActiveId ?? ''))) {
          newActiveId = remaining[0];
        }
      }
    } else {
      // Not in tree — check legacy splitViews directly (defensive)
      const split = splitViews[wsId];
      if (split && (split.leftTerminalId === id || split.rightTerminalId === id)) {
        const { [wsId]: _, ...rest } = splitViews;
        splitViews = rest;
        const remainingId = split.leftTerminalId === id
          ? split.rightTerminalId
          : split.leftTerminalId;
        if (remainingTerminals.some(t => t.id === remainingId)) {
          newActiveId = remainingId;
        }
      }
    }
  }

  store.setState({
    terminals: remainingTerminals,
    activeTerminalId: newActiveId,
    splitViews,
    layoutTrees,
    zoomedPanes,
  });
  store.deleteResumedSession(id);
  store.syncSessionPauseState();
}

export function setActiveTerminalImpl(store: Store, id: string | null): void {
  const state = store.getState();
  if (id && state.activeWorkspaceId) {
    store.setLastActiveTerminal(state.activeWorkspaceId, id);
    const wsId = state.activeWorkspaceId;
    const tree = state.layoutTrees[wsId];

    if (tree) {
      // Check if terminal is in the layout tree
      if (containsTerminal(tree, id)) {
        // Terminal is IN the tree — just update focus, don't clear anything
        // Clear zoom if we're focusing a different pane than the zoomed one
        let zoomedPanes = state.zoomedPanes;
        if (zoomedPanes[wsId] && zoomedPanes[wsId] !== id) {
          const { [wsId]: _, ...rest } = zoomedPanes;
          zoomedPanes = rest;
        }
        store.setState({ activeTerminalId: id, zoomedPanes });
        invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
        store.syncSessionPauseState();
        return;
      }

      // Terminal is NOT in the tree — suspend the tree so it can be restored later
      const { [wsId]: _tree, ...remainingTrees } = state.layoutTrees;
      const { [wsId]: _split, ...remainingSplits } = state.splitViews;
      const { [wsId]: _zoom, ...remainingZooms } = state.zoomedPanes;
      store.setSuspendedLayoutTree(wsId, {
        tree,
        splitView: _split,
        zoomedPane: _zoom,
      });
      store.setState({
        activeTerminalId: id,
        layoutTrees: remainingTrees,
        splitViews: remainingSplits,
        zoomedPanes: remainingZooms,
      });
      invoke('clear_split_view', { workspaceId: wsId }).catch(() => {});
      invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
      store.syncSessionPauseState();
      return;
    }

    // Check if terminal is in a suspended layout tree — restore the split
    const suspended = store.getSuspendedLayoutTree(wsId);
    if (suspended && containsTerminal(suspended.tree, id)) {
      store.deleteSuspendedLayoutTree(wsId);
      const layoutTrees = { ...state.layoutTrees, [wsId]: suspended.tree };
      const splitViews = suspended.splitView
        ? { ...state.splitViews, [wsId]: suspended.splitView }
        : state.splitViews;
      const zoomedPanes = suspended.zoomedPane
        ? { ...state.zoomedPanes, [wsId]: suspended.zoomedPane }
        : state.zoomedPanes;
      store.setState({ activeTerminalId: id, layoutTrees, splitViews, zoomedPanes });
      invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
      store.syncSessionPauseState();
      return;
    }

    // Legacy: If navigating to a terminal outside the current split → suspend
    const split = state.splitViews[wsId];
    if (split && id !== split.leftTerminalId && id !== split.rightTerminalId) {
      const { [wsId]: _, ...remainingSplits } = state.splitViews;
      store.setState({
        activeTerminalId: id,
        splitViews: remainingSplits,
      });
      invoke('clear_split_view', { workspaceId: wsId }).catch(() => {});
      invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
      store.syncSessionPauseState();
      return;
    }
  }
  store.setState({ activeTerminalId: id });
  invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
  store.syncSessionPauseState();
}

export function moveTerminalToWorkspaceImpl(store: Store, terminalId: string, workspaceId: string): void {
  const state = store.getState();
  const terminal = state.terminals.find(t => t.id === terminalId);
  let splitViews = state.splitViews;
  let layoutTrees = state.layoutTrees;
  let zoomedPanes = state.zoomedPanes;

  if (terminal) {
    const srcWs = terminal.workspaceId;
    const tree = layoutTrees[srcWs];

    if (tree && containsTerminal(tree, terminalId)) {
      // Remove from tree
      if (zoomedPanes[srcWs] === terminalId) {
        const { [srcWs]: _, ...rest } = zoomedPanes;
        zoomedPanes = rest;
      }

      const { result } = removeLeaf(tree, terminalId);
      if (!result || result.type === 'leaf') {
        const { [srcWs]: _t, ...restTrees } = layoutTrees;
        const { [srcWs]: _s, ...restSplits } = splitViews;
        layoutTrees = restTrees;
        splitViews = restSplits;
      } else {
        layoutTrees = { ...layoutTrees, [srcWs]: result };
        splitViews = { ...splitViews, ...store.treeToSplitViews(srcWs, result) };
      }
    } else {
      // Legacy fallback
      const split = splitViews[srcWs];
      if (split && (split.leftTerminalId === terminalId || split.rightTerminalId === terminalId)) {
        const { [srcWs]: _, ...rest } = splitViews;
        splitViews = rest;
      }
    }
  }

  store.setState({
    terminals: state.terminals.map(t =>
      t.id === terminalId ? { ...t, workspaceId } : t
    ),
    splitViews,
    layoutTrees,
    zoomedPanes,
  });
}

export function reorderTerminalsImpl(store: Store, workspaceId: string, tabOrder: string[]): void {
  store.setState({
    terminals: store.getState().terminals.map(t => {
      if (t.workspaceId !== workspaceId) return t;
      const order = tabOrder.indexOf(t.id);
      return { ...t, order: order >= 0 ? order : t.order };
    }),
  });
  store.enforceSplitAdjacency(workspaceId);
}

export function togglePinTabImpl(store: Store, terminalId: string): void {
  const state = store.getState();
  const terminal = state.terminals.find(t => t.id === terminalId);
  if (!terminal) return;

  const newPinned = !terminal.pinned;

  // Update the pinned state
  store.setState({
    terminals: state.terminals.map(t =>
      t.id === terminalId ? { ...t, pinned: newPinned } : t
    ),
  });
}
