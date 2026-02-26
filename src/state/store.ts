import { invoke } from '@tauri-apps/api/core';
import {
  LayoutNode,
  terminalIds,
  replaceLeaf,
  removeLeaf,
  containsTerminal,
  updateRatioAtPath,
  swapTerminals,
  findAdjacentTerminal,
} from './split-types';

export type { LayoutNode } from './split-types';

export type PaneType = 'terminal' | 'figma';

export interface Terminal {
  id: string;
  workspaceId: string;
  name: string;
  processName: string;
  order: number;
  oscTitle?: string;
  userRenamed?: boolean;
  paneType?: PaneType;
  figmaUrl?: string;
  exited?: boolean;
  exitCode?: number;
}

export type ShellType =
  | { type: 'windows' }
  | { type: 'pwsh' }
  | { type: 'cmd' }
  | { type: 'wsl'; distribution?: string }
  | { type: 'custom'; program: string; args?: string[] };

export interface Workspace {
  id: string;
  name: string;
  folderPath: string;
  tabOrder: string[];
  shellType: ShellType;
  worktreeMode: boolean;
  claudeCodeMode: boolean;
}

/** Legacy flat split view — kept for backward compatibility. */
export interface SplitView {
  leftTerminalId: string;   // or "top" in vertical
  rightTerminalId: string;  // or "bottom" in vertical
  direction: 'horizontal' | 'vertical';
  ratio: number;            // 0.0-1.0, default 0.5
}

export interface AppState {
  workspaces: Workspace[];
  terminals: Terminal[];
  activeWorkspaceId: string | null;
  activeTerminalId: string | null;
  /** @deprecated Use layoutTrees instead — kept for backward compatibility. */
  splitViews: Record<string, SplitView>;
  /** Recursive layout trees, keyed by workspaceId. */
  layoutTrees: Record<string, LayoutNode>;
  /** Zoomed pane per workspace — when set, this pane fills the entire area. */
  zoomedPanes: Record<string, string>;
}

type Listener = () => void;

class Store {

  private state: AppState = {
    workspaces: [],
    terminals: [],
    activeWorkspaceId: null,
    activeTerminalId: null,
    splitViews: {},
    layoutTrees: {},
    zoomedPanes: {},
  };

  private listeners: Set<Listener> = new Set();
  private lastActiveTerminalByWorkspace: Map<string, string> = new Map();
  private pendingNotify = false;
  /** Sessions currently resumed (not paused). Tracks which sessions we've
   *  sent resumeSession to, so we can pause them when they become invisible. */
  private resumedSessions: Set<string> = new Set();

  getState(): AppState {
    return this.state;
  }

  setState(partial: Partial<AppState>) {
    this.state = { ...this.state, ...partial };
    this.notify();
  }

  reset() {
    this.state = {
      workspaces: [],
      terminals: [],
      activeWorkspaceId: null,
      activeTerminalId: null,
      splitViews: {},
      layoutTrees: {},
      zoomedPanes: {},
    };
    this.lastActiveTerminalByWorkspace.clear();
    this.resumedSessions.clear();
    this.notify();
  }

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify() {
    if (typeof requestAnimationFrame === 'function') {
      // Browser: coalesce multiple setState() calls within a single frame
      if (!this.pendingNotify) {
        this.pendingNotify = true;
        requestAnimationFrame(() => {
          this.pendingNotify = false;
          this.listeners.forEach(listener => listener());
        });
      }
    } else {
      // Non-browser (tests): notify synchronously
      this.listeners.forEach(listener => listener());
    }
  }

  // ---------------------------------------------------------------------------
  // Workspace operations
  // ---------------------------------------------------------------------------

  addWorkspace(workspace: Workspace) {
    this.setState({
      workspaces: [...this.state.workspaces, workspace],
    });
  }

  updateWorkspace(id: string, updates: Partial<Workspace>) {
    this.setState({
      workspaces: this.state.workspaces.map(w =>
        w.id === id ? { ...w, ...updates } : w
      ),
    });
  }

  removeWorkspace(id: string) {
    this.lastActiveTerminalByWorkspace.delete(id);
    const { [id]: _s, ...remainingSplitViews } = this.state.splitViews;
    const { [id]: _t, ...remainingTrees } = this.state.layoutTrees;
    const { [id]: _z, ...remainingZoomed } = this.state.zoomedPanes;
    this.setState({
      workspaces: this.state.workspaces.filter(w => w.id !== id),
      terminals: this.state.terminals.filter(t => t.workspaceId !== id),
      activeWorkspaceId: this.state.activeWorkspaceId === id
        ? (this.state.workspaces[0]?.id ?? null)
        : this.state.activeWorkspaceId,
      splitViews: remainingSplitViews,
      layoutTrees: remainingTrees,
      zoomedPanes: remainingZoomed,
    });
  }

  setActiveWorkspace(id: string | null) {
    // Remember current active terminal for the workspace we're leaving
    if (this.state.activeWorkspaceId && this.state.activeTerminalId) {
      this.lastActiveTerminalByWorkspace.set(
        this.state.activeWorkspaceId,
        this.state.activeTerminalId,
      );
    }

    const workspaceTerminals = this.state.terminals.filter(t => t.workspaceId === id);
    const rememberedId = id ? this.lastActiveTerminalByWorkspace.get(id) : null;
    const rememberedStillExists = rememberedId && workspaceTerminals.some(t => t.id === rememberedId);

    this.setState({
      activeWorkspaceId: id,
      activeTerminalId: rememberedStillExists ? rememberedId : (workspaceTerminals[0]?.id ?? null),
    });
    this.syncSessionPauseState();
  }

  // ---------------------------------------------------------------------------
  // Terminal operations
  // ---------------------------------------------------------------------------

  addTerminal(terminal: Terminal, opts?: { background?: boolean }) {
    const workspaceTerminals = this.state.terminals.filter(
      t => t.workspaceId === terminal.workspaceId
    );
    const order = workspaceTerminals.length;

    if (opts?.background) {
      this.setState({
        terminals: [...this.state.terminals, { ...terminal, order }],
      });
    } else {
      this.lastActiveTerminalByWorkspace.set(terminal.workspaceId, terminal.id);
      this.setState({
        terminals: [...this.state.terminals, { ...terminal, order }],
        activeTerminalId: terminal.id,
      });
    }
    this.syncSessionPauseState();
  }

  updateTerminal(id: string, updates: Partial<Terminal>) {
    const existing = this.state.terminals.find(t => t.id === id);
    if (!existing) return;

    // Skip if no values actually changed
    const changed = Object.entries(updates).some(
      ([key, value]) => existing[key as keyof Terminal] !== value
    );
    if (!changed) return;

    this.setState({
      terminals: this.state.terminals.map(t =>
        t.id === id ? { ...t, ...updates } : t
      ),
    });
  }

  removeTerminal(id: string) {
    const terminal = this.state.terminals.find(t => t.id === id);
    const remainingTerminals = this.state.terminals.filter(t => t.id !== id);

    let newActiveId = this.state.activeTerminalId;
    if (this.state.activeTerminalId === id && terminal) {
      const sameWorkspace = remainingTerminals.filter(
        t => t.workspaceId === terminal.workspaceId
      );
      newActiveId = sameWorkspace[0]?.id ?? null;
    }

    let layoutTrees = this.state.layoutTrees;
    let splitViews = this.state.splitViews;
    let zoomedPanes = this.state.zoomedPanes;

    if (terminal) {
      const wsId = terminal.workspaceId;
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
          splitViews = { ...splitViews, ...this.treeToSplitViews(wsId, result) };
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

    this.setState({
      terminals: remainingTerminals,
      activeTerminalId: newActiveId,
      splitViews,
      layoutTrees,
      zoomedPanes,
    });
    this.resumedSessions.delete(id);
    this.syncSessionPauseState();
  }

  setActiveTerminal(id: string | null) {
    if (id && this.state.activeWorkspaceId) {
      this.lastActiveTerminalByWorkspace.set(this.state.activeWorkspaceId, id);
      const wsId = this.state.activeWorkspaceId;
      const tree = this.state.layoutTrees[wsId];

      if (tree) {
        // Check if terminal is in the layout tree
        if (containsTerminal(tree, id)) {
          // Terminal is IN the tree — just update focus, don't clear anything
          // Clear zoom if we're focusing a different pane than the zoomed one
          let zoomedPanes = this.state.zoomedPanes;
          if (zoomedPanes[wsId] && zoomedPanes[wsId] !== id) {
            const { [wsId]: _, ...rest } = zoomedPanes;
            zoomedPanes = rest;
          }
          this.setState({ activeTerminalId: id, zoomedPanes });
          invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
          this.syncSessionPauseState();
          return;
        }

        // Terminal is NOT in the tree — replace the active pane in the tree
        const activeId = this.state.activeTerminalId;
        if (activeId && containsTerminal(tree, activeId)) {
          const newTree = replaceLeaf(tree, activeId, { type: 'leaf', terminal_id: id });
          if (newTree) {
            const zoomedPanes = { ...this.state.zoomedPanes };
            if (zoomedPanes[wsId]) {
              delete zoomedPanes[wsId];
            }
            this.setState({
              activeTerminalId: id,
              layoutTrees: { ...this.state.layoutTrees, [wsId]: newTree },
              splitViews: { ...this.state.splitViews, ...this.treeToSplitViews(wsId, newTree) },
              zoomedPanes,
            });
            invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
            this.enforceSplitAdjacency(wsId);
            this.syncSessionPauseState();
            return;
          }
        }

        // Active terminal isn't in the tree either — replace second child's first leaf
        const ids = terminalIds(tree);
        if (ids.length > 0) {
          // Replace the last leaf (rightmost/bottommost)
          const lastId = ids[ids.length - 1];
          const newTree = replaceLeaf(tree, lastId, { type: 'leaf', terminal_id: id });
          if (newTree) {
            const zoomedPanes = { ...this.state.zoomedPanes };
            if (zoomedPanes[wsId]) {
              delete zoomedPanes[wsId];
            }
            this.setState({
              activeTerminalId: id,
              layoutTrees: { ...this.state.layoutTrees, [wsId]: newTree },
              splitViews: { ...this.state.splitViews, ...this.treeToSplitViews(wsId, newTree) },
              zoomedPanes,
            });
            invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
            this.enforceSplitAdjacency(wsId);
            this.syncSessionPauseState();
            return;
          }
        }
      }

      // Legacy: handle splitViews without a layout tree (shouldn't happen normally)
      const split = this.state.splitViews[wsId];
      if (split && id !== split.leftTerminalId && id !== split.rightTerminalId) {
        const activeId = this.state.activeTerminalId;
        let newLeft = split.leftTerminalId;
        let newRight = split.rightTerminalId;

        if (activeId === split.leftTerminalId) {
          newLeft = id;
        } else if (activeId === split.rightTerminalId) {
          newRight = id;
        } else {
          newRight = id;
        }

        const updatedSplit = { ...split, leftTerminalId: newLeft, rightTerminalId: newRight };
        this.setState({
          activeTerminalId: id,
          splitViews: { ...this.state.splitViews, [wsId]: updatedSplit },
        });
        invoke('set_split_view', {
          workspaceId: wsId,
          leftTerminalId: newLeft,
          rightTerminalId: newRight,
          direction: split.direction,
          ratio: split.ratio,
        }).catch(() => {});
        invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
        this.enforceSplitAdjacency(wsId);
        this.syncSessionPauseState();
        return;
      }
    }
    this.setState({ activeTerminalId: id });
    invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
    this.syncSessionPauseState();
  }

  moveTerminalToWorkspace(terminalId: string, workspaceId: string) {
    const terminal = this.state.terminals.find(t => t.id === terminalId);
    let splitViews = this.state.splitViews;
    let layoutTrees = this.state.layoutTrees;
    let zoomedPanes = this.state.zoomedPanes;

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
          splitViews = { ...splitViews, ...this.treeToSplitViews(srcWs, result) };
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

    this.setState({
      terminals: this.state.terminals.map(t =>
        t.id === terminalId ? { ...t, workspaceId } : t
      ),
      splitViews,
      layoutTrees,
      zoomedPanes,
    });
  }

  reorderWorkspaces(workspaceIds: string[]) {
    const workspaceMap = new Map(this.state.workspaces.map(w => [w.id, w]));
    const reordered = workspaceIds
      .map(id => workspaceMap.get(id))
      .filter((w): w is Workspace => w !== undefined);
    this.setState({ workspaces: reordered });
  }

  reorderTerminals(workspaceId: string, tabOrder: string[]) {
    this.setState({
      terminals: this.state.terminals.map(t => {
        if (t.workspaceId !== workspaceId) return t;
        const order = tabOrder.indexOf(t.id);
        return { ...t, order: order >= 0 ? order : t.order };
      }),
    });
    this.enforceSplitAdjacency(workspaceId);
  }

  // ---------------------------------------------------------------------------
  // Layout tree operations
  // ---------------------------------------------------------------------------

  /** Get the layout tree for a workspace. */
  getLayoutTree(workspaceId: string): LayoutNode | null {
    return this.state.layoutTrees[workspaceId] ?? null;
  }

  /** Set the layout tree for a workspace. Also syncs the legacy splitViews. */
  setLayoutTree(workspaceId: string, tree: LayoutNode): void {
    this.setState({
      layoutTrees: { ...this.state.layoutTrees, [workspaceId]: tree },
      splitViews: { ...this.state.splitViews, ...this.treeToSplitViews(workspaceId, tree) },
    });
    this.enforceSplitAdjacency(workspaceId);
  }

  /** Clear the layout tree for a workspace. */
  clearLayoutTree(workspaceId: string): void {
    const { [workspaceId]: _t, ...restTrees } = this.state.layoutTrees;
    const { [workspaceId]: _s, ...restSplits } = this.state.splitViews;
    const { [workspaceId]: _z, ...restZoomed } = this.state.zoomedPanes;
    this.setState({
      layoutTrees: restTrees,
      splitViews: restSplits,
      zoomedPanes: restZoomed,
    });
  }

  /**
   * Split a terminal at a target leaf, creating a new pane.
   * If no tree exists, creates one with the target as the first leaf.
   */
  splitTerminalAt(
    workspaceId: string,
    targetTerminalId: string,
    newTerminalId: string,
    direction: 'horizontal' | 'vertical',
    ratio = 0.5,
  ): void {
    const tree = this.state.layoutTrees[workspaceId];
    const newSplit: LayoutNode = {
      type: 'split',
      direction,
      ratio,
      first: { type: 'leaf', terminal_id: targetTerminalId },
      second: { type: 'leaf', terminal_id: newTerminalId },
    };

    if (!tree) {
      // No tree yet — create one
      this.setLayoutTree(workspaceId, newSplit);
    } else {
      // Replace the target leaf with a split node
      const newTree = replaceLeaf(tree, targetTerminalId, newSplit);
      if (newTree) {
        this.setLayoutTree(workspaceId, newTree);
      }
    }
  }

  /**
   * Remove a terminal from the layout tree, collapsing the split.
   * If the tree collapses to a single leaf, clears the tree entirely.
   */
  unsplitTerminal(workspaceId: string, terminalId: string): void {
    const tree = this.state.layoutTrees[workspaceId];
    if (!tree) return;

    if (!containsTerminal(tree, terminalId)) return;

    // Clear zoom if needed
    let zoomedPanes = this.state.zoomedPanes;
    if (zoomedPanes[workspaceId]) {
      const { [workspaceId]: _, ...rest } = zoomedPanes;
      zoomedPanes = rest;
    }

    const { result } = removeLeaf(tree, terminalId);
    if (!result || result.type === 'leaf') {
      // Collapsed to single leaf or empty — clear tree
      this.clearLayoutTree(workspaceId);
      this.setState({ zoomedPanes });
    } else {
      this.setState({
        layoutTrees: { ...this.state.layoutTrees, [workspaceId]: result },
        splitViews: { ...this.state.splitViews, ...this.treeToSplitViews(workspaceId, result) },
        zoomedPanes,
      });
      this.enforceSplitAdjacency(workspaceId);
    }
  }

  // ---------------------------------------------------------------------------
  // Navigation
  // ---------------------------------------------------------------------------

  /** Return the currently active terminal if it's in the layout tree. */
  getFocusedPaneId(workspaceId: string): string | null {
    const tree = this.state.layoutTrees[workspaceId];
    if (!tree) return null;
    const activeId = this.state.activeTerminalId;
    if (activeId && containsTerminal(tree, activeId)) return activeId;
    return null;
  }

  /** Walk the tree to find the nearest terminal in the given direction. */
  getAdjacentPane(
    workspaceId: string,
    terminalId: string,
    direction: 'horizontal' | 'vertical',
    goSecond: boolean,
  ): string | null {
    const tree = this.state.layoutTrees[workspaceId];
    if (!tree) return null;
    return findAdjacentTerminal(tree, terminalId, direction, goSecond);
  }

  // ---------------------------------------------------------------------------
  // Resize
  // ---------------------------------------------------------------------------

  /**
   * Update the ratio of a split node identified by path.
   * Path is an array of indices: 0 = first child, 1 = second child.
   * Ratio is clamped to [0.15, 0.85].
   */
  updateTreeRatio(workspaceId: string, path: number[], ratio: number): void {
    const tree = this.state.layoutTrees[workspaceId];
    if (!tree) return;

    const clamped = Math.max(0.15, Math.min(0.85, ratio));
    const updated = updateRatioAtPath(tree, path, clamped);
    if (updated) {
      this.setState({
        layoutTrees: { ...this.state.layoutTrees, [workspaceId]: updated },
        splitViews: { ...this.state.splitViews, ...this.treeToSplitViews(workspaceId, updated) },
      });
    }
  }

  // ---------------------------------------------------------------------------
  // Zoom
  // ---------------------------------------------------------------------------

  /** Zoom a pane to fill the entire area. Pass null to unzoom. */
  setZoomedPane(workspaceId: string, terminalId: string | null): void {
    if (terminalId === null) {
      const { [workspaceId]: _, ...rest } = this.state.zoomedPanes;
      this.setState({ zoomedPanes: rest });
    } else {
      this.setState({
        zoomedPanes: { ...this.state.zoomedPanes, [workspaceId]: terminalId },
      });
    }
  }

  /** Get the currently zoomed pane for a workspace, or null. */
  getZoomedPane(workspaceId: string): string | null {
    return this.state.zoomedPanes[workspaceId] ?? null;
  }

  // ---------------------------------------------------------------------------
  // Swap
  // ---------------------------------------------------------------------------

  /** Swap two terminal panes in the layout tree. */
  swapPanes(workspaceId: string, idA: string, idB: string): void {
    const tree = this.state.layoutTrees[workspaceId];
    if (!tree) return;

    const swapped = swapTerminals(tree, idA, idB);
    if (swapped) {
      this.setState({
        layoutTrees: { ...this.state.layoutTrees, [workspaceId]: swapped },
        splitViews: { ...this.state.splitViews, ...this.treeToSplitViews(workspaceId, swapped) },
      });
      this.enforceSplitAdjacency(workspaceId);
    }
  }

  // ---------------------------------------------------------------------------
  // Legacy split view operations (backward compatibility wrappers)
  // ---------------------------------------------------------------------------

  /**
   * Create a split view. Internally creates a 2-leaf layout tree.
   * @deprecated Use setLayoutTree or splitTerminalAt instead.
   */
  setSplitView(
    workspaceId: string,
    leftTerminalId: string,
    rightTerminalId: string,
    direction: 'horizontal' | 'vertical',
    ratio = 0.5,
  ) {
    const tree: LayoutNode = {
      type: 'split',
      direction,
      ratio,
      first: { type: 'leaf', terminal_id: leftTerminalId },
      second: { type: 'leaf', terminal_id: rightTerminalId },
    };
    this.setLayoutTree(workspaceId, tree);
  }

  /**
   * Clear a split view.
   * @deprecated Use clearLayoutTree instead.
   */
  clearSplitView(workspaceId: string) {
    this.clearLayoutTree(workspaceId);
  }

  /**
   * Get the legacy split view for a workspace.
   * Only returns data for simple 2-pane splits. Returns null for 3+ pane trees.
   * @deprecated Use getLayoutTree instead.
   */
  getSplitView(workspaceId: string): SplitView | null {
    const tree = this.state.layoutTrees[workspaceId];
    if (!tree || tree.type !== 'split') return null;
    if (tree.first.type !== 'leaf' || tree.second.type !== 'leaf') return null;
    return {
      leftTerminalId: tree.first.terminal_id,
      rightTerminalId: tree.second.terminal_id,
      direction: tree.direction,
      ratio: tree.ratio,
    };
  }

  /**
   * Update split ratio.
   * @deprecated Use updateTreeRatio instead.
   */
  updateSplitRatio(workspaceId: string, ratio: number) {
    const tree = this.state.layoutTrees[workspaceId];
    if (!tree || tree.type !== 'split') return;
    this.setState({
      layoutTrees: {
        ...this.state.layoutTrees,
        [workspaceId]: { ...tree, ratio },
      },
      splitViews: {
        ...this.state.splitViews,
        [workspaceId]: { ...this.state.splitViews[workspaceId], ratio },
      },
    });
  }

  // ---------------------------------------------------------------------------
  // Internal helpers
  // ---------------------------------------------------------------------------

  /**
   * Ensure tree-leaf terminals are adjacent in tab order.
   * Orders tabs to match depth-first traversal of tree leaves.
   */
  private enforceSplitAdjacency(workspaceId: string) {
    const tree = this.state.layoutTrees[workspaceId];
    if (!tree) {
      // Legacy fallback (shouldn't happen, but defensive)
      return;
    }

    const treeIds = terminalIds(tree);
    if (treeIds.length < 2) return;

    const wsTerminals = this.getWorkspaceTerminals(workspaceId);
    const ids = wsTerminals.map(t => t.id);

    // Check if the tree IDs already appear in order and adjacent
    const positions = treeIds.map(tid => ids.indexOf(tid)).filter(i => i !== -1);
    if (positions.length < 2) return;

    // Check if already adjacent and in DFS order
    let alreadyCorrect = true;
    for (let i = 1; i < positions.length; i++) {
      if (positions[i] !== positions[i - 1] + 1) {
        alreadyCorrect = false;
        break;
      }
    }
    if (alreadyCorrect) return;

    // Build new order: place tree IDs adjacent starting at the earliest position
    const nonTreeIds = ids.filter(i => !treeIds.includes(i));
    const insertPos = Math.min(...positions);
    const newOrder = [
      ...nonTreeIds.slice(0, insertPos),
      ...treeIds,
      ...nonTreeIds.slice(insertPos),
    ];

    this.setState({
      terminals: this.state.terminals.map(t => {
        if (t.workspaceId !== workspaceId) return t;
        const order = newOrder.indexOf(t.id);
        return { ...t, order: order >= 0 ? order : t.order };
      }),
    });
  }

  /**
   * Sync session pause/resume state based on currently visible terminals.
   * Visible terminals are resumed, all others are paused.
   * This reduces daemon output overhead for background sessions.
   */
  syncSessionPauseState() {
    const { activeTerminalId, activeWorkspaceId, layoutTrees } = this.state;
    const visibleIds = new Set<string>();

    if (activeTerminalId) {
      visibleIds.add(activeTerminalId);
    }

    // Include all terminals from the layout tree
    if (activeWorkspaceId) {
      const tree = layoutTrees[activeWorkspaceId];
      if (tree) {
        for (const id of terminalIds(tree)) {
          visibleIds.add(id);
        }
      }
    }

    // Resume visible, pause invisible
    for (const terminal of this.state.terminals) {
      if (visibleIds.has(terminal.id)) {
        if (!this.resumedSessions.has(terminal.id)) {
          this.resumedSessions.add(terminal.id);
          invoke('resume_session', { sessionId: terminal.id }).catch(() => {});
        }
      } else {
        if (this.resumedSessions.has(terminal.id)) {
          this.resumedSessions.delete(terminal.id);
          invoke('pause_session', { sessionId: terminal.id }).catch(() => {});
        }
      }
    }
  }

  /**
   * Convert a layout tree to a legacy SplitView record (for backward compat).
   * Only produces a SplitView if the tree is a simple 2-leaf split.
   * For nested trees (3+ panes), removes the workspace from splitViews.
   */
  private treeToSplitViews(workspaceId: string, tree: LayoutNode): Record<string, SplitView> | null {
    if (tree.type !== 'split') return null;
    if (tree.first.type === 'leaf' && tree.second.type === 'leaf') {
      return {
        [workspaceId]: {
          leftTerminalId: tree.first.terminal_id,
          rightTerminalId: tree.second.terminal_id,
          direction: tree.direction,
          ratio: tree.ratio,
        },
      };
    }
    // Nested trees can't be represented as a flat SplitView
    return null;
  }

  getWorkspaceTerminals(workspaceId: string): Terminal[] {
    return this.state.terminals
      .filter(t => t.workspaceId === workspaceId)
      .sort((a, b) => a.order - b.order);
  }

  getTerminalCount(workspaceId: string): number {
    return this.state.terminals.filter(t => t.workspaceId === workspaceId).length;
  }

  getVisibleWorkspaces(): Workspace[] {
    return this.state.workspaces;
  }
}

export const store = new Store();
