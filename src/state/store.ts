import { invoke } from '@tauri-apps/api/core';
import {
  LayoutNode,
  terminalIds,
} from './split-types';
import {
  addWorkspaceImpl,
  updateWorkspaceImpl,
  removeWorkspaceImpl,
  setActiveWorkspaceImpl,
  reorderWorkspacesImpl,
  getWorkspaceTerminalsImpl,
  getVisibleWorkspacesImpl,
} from './store-workspace';
import {
  addTerminalImpl,
  updateTerminalImpl,
  removeTerminalImpl,
  setActiveTerminalImpl,
  moveTerminalToWorkspaceImpl,
  reorderTerminalsImpl,
} from './store-terminal';
import {
  getLayoutTreeImpl,
  setLayoutTreeImpl,
  clearLayoutTreeImpl,
  splitTerminalAtImpl,
  unsplitTerminalImpl,
  getFocusedPaneIdImpl,
  getAdjacentPaneImpl,
  updateTreeRatioImpl,
  setZoomedPaneImpl,
  getZoomedPaneImpl,
  swapPanesImpl,
  setSplitViewImpl,
  clearSplitViewImpl,
  getSplitViewImpl,
  updateSplitRatioImpl,
} from './store-layout';

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

/** Built-in modes plus custom tool IDs (e.g. 'custom-1234'). */
export type AiToolMode = 'none' | 'claude' | 'codex' | 'both' | (string & {});

export interface Workspace {
  id: string;
  name: string;
  folderPath: string;
  tabOrder: string[];
  shellType: ShellType;
  worktreeMode: boolean;
  aiToolMode: AiToolMode;
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

export class Store {

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
  private previousActiveTerminalByWorkspace: Map<string, string> = new Map();
  private pendingNotify = false;
  /** Suspended layout trees, keyed by workspaceId. Stored when navigating to a
   *  tab outside the split so the split can be restored on return. */
  private suspendedLayoutTrees: Map<string, { tree: LayoutNode; splitView?: SplitView; zoomedPane?: string }> = new Map();
  /** Sessions currently resumed (not paused). Tracks which sessions we've
   *  sent resumeSession to, so we can pause them when they become invisible. */
  private resumedSessions: Set<string> = new Set();

  // ---------------------------------------------------------------------------
  // Core state management
  // ---------------------------------------------------------------------------

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
    this.previousActiveTerminalByWorkspace.clear();
    this.resumedSessions.clear();
    this.suspendedLayoutTrees.clear();
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
  // Private state accessors (bridge for domain modules)
  // ---------------------------------------------------------------------------

  getLastActiveTerminal(wsId: string): string | null {
    return this.lastActiveTerminalByWorkspace.get(wsId) ?? null;
  }

  setLastActiveTerminal(wsId: string, termId: string): void {
    const current = this.lastActiveTerminalByWorkspace.get(wsId);
    if (current && current !== termId) {
      this.previousActiveTerminalByWorkspace.set(wsId, current);
    }
    this.lastActiveTerminalByWorkspace.set(wsId, termId);
  }

  getPreviousActiveTerminal(wsId: string): string | null {
    return this.previousActiveTerminalByWorkspace.get(wsId) ?? null;
  }

  deleteLastActiveTerminal(wsId: string): void {
    this.lastActiveTerminalByWorkspace.delete(wsId);
    this.previousActiveTerminalByWorkspace.delete(wsId);
  }

  getSuspendedLayoutTree(wsId: string): { tree: LayoutNode; splitView?: SplitView; zoomedPane?: string } | undefined {
    return this.suspendedLayoutTrees.get(wsId);
  }

  setSuspendedLayoutTree(wsId: string, data: { tree: LayoutNode; splitView?: SplitView; zoomedPane?: string }): void {
    this.suspendedLayoutTrees.set(wsId, data);
  }

  deleteSuspendedLayoutTree(wsId: string): void {
    this.suspendedLayoutTrees.delete(wsId);
  }

  addResumedSession(id: string): void {
    this.resumedSessions.add(id);
  }

  deleteResumedSession(id: string): void {
    this.resumedSessions.delete(id);
  }

  hasResumedSession(id: string): boolean {
    return this.resumedSessions.has(id);
  }

  // ---------------------------------------------------------------------------
  // Workspace operations (delegated to store-workspace.ts)
  // ---------------------------------------------------------------------------

  addWorkspace(workspace: Workspace) { addWorkspaceImpl(this, workspace); }
  updateWorkspace(id: string, updates: Partial<Workspace>) { updateWorkspaceImpl(this, id, updates); }
  removeWorkspace(id: string) { removeWorkspaceImpl(this, id); }
  setActiveWorkspace(id: string | null) { setActiveWorkspaceImpl(this, id); }
  reorderWorkspaces(workspaceIds: string[]) { reorderWorkspacesImpl(this, workspaceIds); }
  getWorkspaceTerminals(workspaceId: string): Terminal[] { return getWorkspaceTerminalsImpl(this, workspaceId); }
  getVisibleWorkspaces(): Workspace[] { return getVisibleWorkspacesImpl(this); }

  // ---------------------------------------------------------------------------
  // Terminal operations (delegated to store-terminal.ts)
  // ---------------------------------------------------------------------------

  addTerminal(terminal: Terminal, opts?: { background?: boolean }) { addTerminalImpl(this, terminal, opts); }
  updateTerminal(id: string, updates: Partial<Terminal>) { updateTerminalImpl(this, id, updates); }
  removeTerminal(id: string) { removeTerminalImpl(this, id); }
  setActiveTerminal(id: string | null) { setActiveTerminalImpl(this, id); }
  moveTerminalToWorkspace(terminalId: string, workspaceId: string) { moveTerminalToWorkspaceImpl(this, terminalId, workspaceId); }
  reorderTerminals(workspaceId: string, tabOrder: string[]) { reorderTerminalsImpl(this, workspaceId, tabOrder); }

  // ---------------------------------------------------------------------------
  // Layout tree operations (delegated to store-layout.ts)
  // ---------------------------------------------------------------------------

  getLayoutTree(workspaceId: string): LayoutNode | null { return getLayoutTreeImpl(this, workspaceId); }
  setLayoutTree(workspaceId: string, tree: LayoutNode): void { setLayoutTreeImpl(this, workspaceId, tree); }
  clearLayoutTree(workspaceId: string): void { clearLayoutTreeImpl(this, workspaceId); }
  splitTerminalAt(workspaceId: string, targetTerminalId: string, newTerminalId: string, direction: 'horizontal' | 'vertical', ratio = 0.5): void {
    splitTerminalAtImpl(this, workspaceId, targetTerminalId, newTerminalId, direction, ratio);
  }
  unsplitTerminal(workspaceId: string, terminalId?: string): void { unsplitTerminalImpl(this, workspaceId, terminalId); }
  getFocusedPaneId(workspaceId: string): string | null { return getFocusedPaneIdImpl(this, workspaceId); }
  getAdjacentPane(workspaceId: string, terminalId: string, direction: 'horizontal' | 'vertical', goSecond: boolean): string | null {
    return getAdjacentPaneImpl(this, workspaceId, terminalId, direction, goSecond);
  }
  updateTreeRatio(workspaceId: string, path: number[], ratio: number): void { updateTreeRatioImpl(this, workspaceId, path, ratio); }
  updateLayoutTreeRatio(workspaceId: string, path: number[], ratio: number): void { this.updateTreeRatio(workspaceId, path, ratio); }
  setZoomedPane(workspaceId: string, terminalId: string | null): void { setZoomedPaneImpl(this, workspaceId, terminalId); }
  getZoomedPane(workspaceId: string): string | null { return getZoomedPaneImpl(this, workspaceId); }
  swapPanes(workspaceId: string, idA: string, idB: string): void { swapPanesImpl(this, workspaceId, idA, idB); }

  // Legacy split view wrappers
  /** @deprecated Use setLayoutTree or splitTerminalAt instead. */
  setSplitView(workspaceId: string, leftTerminalId: string, rightTerminalId: string, direction: 'horizontal' | 'vertical', ratio = 0.5) {
    setSplitViewImpl(this, workspaceId, leftTerminalId, rightTerminalId, direction, ratio);
  }
  /** @deprecated Use clearLayoutTree instead. */
  clearSplitView(workspaceId: string) { clearSplitViewImpl(this, workspaceId); }
  /** @deprecated Use getLayoutTree instead. */
  getSplitView(workspaceId: string): SplitView | null { return getSplitViewImpl(this, workspaceId); }
  /** @deprecated Use updateTreeRatio instead. */
  updateSplitRatio(workspaceId: string, ratio: number) { updateSplitRatioImpl(this, workspaceId, ratio); }

  // ---------------------------------------------------------------------------
  // Cross-domain helpers (kept in store.ts)
  // ---------------------------------------------------------------------------

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
  treeToSplitViews(workspaceId: string, tree: LayoutNode): Record<string, SplitView> | null {
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

  /**
   * Ensure tree-leaf terminals are adjacent in tab order.
   * Orders tabs to match depth-first traversal of tree leaves.
   */
  enforceSplitAdjacency(workspaceId: string) {
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

  getTerminalCount(workspaceId: string): number {
    return this.state.terminals.filter(t => t.workspaceId === workspaceId).length;
  }
}

export const store = new Store();
