import type { Store, Workspace } from './store';

// ---------------------------------------------------------------------------
// Workspace operations
// ---------------------------------------------------------------------------

export function addWorkspaceImpl(store: Store, workspace: Workspace): void {
  store.setState({
    workspaces: [...store.getState().workspaces, workspace],
  });
}

export function updateWorkspaceImpl(store: Store, id: string, updates: Partial<Workspace>): void {
  store.setState({
    workspaces: store.getState().workspaces.map(w =>
      w.id === id ? { ...w, ...updates } : w
    ),
  });
}

export function removeWorkspaceImpl(store: Store, id: string): void {
  store.deleteLastActiveTerminal(id);
  const { [id]: _s, ...remainingSplitViews } = store.getState().splitViews;
  const { [id]: _t, ...remainingTrees } = store.getState().layoutTrees;
  const { [id]: _z, ...remainingZoomed } = store.getState().zoomedPanes;
  store.setState({
    workspaces: store.getState().workspaces.filter(w => w.id !== id),
    terminals: store.getState().terminals.filter(t => t.workspaceId !== id),
    activeWorkspaceId: store.getState().activeWorkspaceId === id
      ? (store.getState().workspaces[0]?.id ?? null)
      : store.getState().activeWorkspaceId,
    splitViews: remainingSplitViews,
    layoutTrees: remainingTrees,
    zoomedPanes: remainingZoomed,
  });
}

export function setActiveWorkspaceImpl(store: Store, id: string | null): void {
  const state = store.getState();
  // Remember current active terminal for the workspace we're leaving
  if (state.activeWorkspaceId && state.activeTerminalId) {
    store.setLastActiveTerminal(state.activeWorkspaceId, state.activeTerminalId);
  }

  const workspaceTerminals = state.terminals.filter(t => t.workspaceId === id);
  const rememberedId = id ? store.getLastActiveTerminal(id) : null;
  const rememberedStillExists = rememberedId && workspaceTerminals.some(t => t.id === rememberedId);

  store.setState({
    activeWorkspaceId: id,
    activeTerminalId: rememberedStillExists ? rememberedId : (workspaceTerminals[0]?.id ?? null),
  });
  store.syncSessionPauseState();
}

export function reorderWorkspacesImpl(store: Store, workspaceIds: string[]): void {
  const workspaceMap = new Map(store.getState().workspaces.map(w => [w.id, w]));
  const reordered = workspaceIds
    .map(id => workspaceMap.get(id))
    .filter((w): w is Workspace => w !== undefined);
  store.setState({ workspaces: reordered });
}

export function getWorkspaceTerminalsImpl(store: Store, workspaceId: string) {
  return store.getState().terminals
    .filter(t => t.workspaceId === workspaceId)
    .sort((a, b) => a.order - b.order);
}

export function getVisibleWorkspacesImpl(store: Store) {
  return store.getState().workspaces;
}
