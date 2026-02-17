import { invoke } from '@tauri-apps/api/core';

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
  splitViews: Record<string, SplitView>;  // keyed by workspaceId
}

export type WindowMode = 'main' | 'mcp';

type Listener = () => void;

function detectWindowMode(): WindowMode {
  try {
    const params = new URLSearchParams(window.location.search);
    return params.get('mode') === 'mcp' ? 'mcp' : 'main';
  } catch {
    return 'main';
  }
}

class Store {
  readonly windowMode: WindowMode = detectWindowMode();

  private state: AppState = {
    workspaces: [],
    terminals: [],
    activeWorkspaceId: null,
    activeTerminalId: null,
    splitViews: {},
  };

  private listeners: Set<Listener> = new Set();
  private lastActiveTerminalByWorkspace: Map<string, string> = new Map();
  private suspendedSplitViews: Map<string, SplitView> = new Map();
  private pendingNotify = false;

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
    };
    this.lastActiveTerminalByWorkspace.clear();
    this.suspendedSplitViews.clear();
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

  // Workspace operations
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
    this.suspendedSplitViews.delete(id);
    const { [id]: _, ...remainingSplitViews } = this.state.splitViews;
    this.setState({
      workspaces: this.state.workspaces.filter(w => w.id !== id),
      terminals: this.state.terminals.filter(t => t.workspaceId !== id),
      activeWorkspaceId: this.state.activeWorkspaceId === id
        ? (this.state.workspaces[0]?.id ?? null)
        : this.state.activeWorkspaceId,
      splitViews: remainingSplitViews,
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
  }

  // Terminal operations
  addTerminal(terminal: Terminal) {
    const workspaceTerminals = this.state.terminals.filter(
      t => t.workspaceId === terminal.workspaceId
    );
    const order = workspaceTerminals.length;

    this.lastActiveTerminalByWorkspace.set(terminal.workspaceId, terminal.id);
    this.setState({
      terminals: [...this.state.terminals, { ...terminal, order }],
      activeTerminalId: terminal.id,
    });
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

    // Clear split (active or suspended) if removed terminal was part of one
    let splitViews = this.state.splitViews;
    if (terminal) {
      const split = splitViews[terminal.workspaceId];
      if (split && (split.leftTerminalId === id || split.rightTerminalId === id)) {
        const { [terminal.workspaceId]: _, ...rest } = splitViews;
        splitViews = rest;
        // Set remaining split terminal as active
        const remainingId = split.leftTerminalId === id
          ? split.rightTerminalId
          : split.leftTerminalId;
        if (remainingTerminals.some(t => t.id === remainingId)) {
          newActiveId = remainingId;
        }
      }

      const suspended = this.suspendedSplitViews.get(terminal.workspaceId);
      if (suspended && (suspended.leftTerminalId === id || suspended.rightTerminalId === id)) {
        this.suspendedSplitViews.delete(terminal.workspaceId);
      }
    }

    this.setState({
      terminals: remainingTerminals,
      activeTerminalId: newActiveId,
      splitViews,
    });
  }

  setActiveTerminal(id: string | null) {
    if (id && this.state.activeWorkspaceId) {
      this.lastActiveTerminalByWorkspace.set(this.state.activeWorkspaceId, id);
      const wsId = this.state.activeWorkspaceId;

      // If navigating to a terminal outside the current split → suspend the split
      const split = this.state.splitViews[wsId];
      if (split && id !== split.leftTerminalId && id !== split.rightTerminalId) {
        this.suspendedSplitViews.set(wsId, split);
        const { [wsId]: _, ...rest } = this.state.splitViews;
        this.setState({ activeTerminalId: id, splitViews: rest });
        invoke('clear_split_view', { workspaceId: wsId }).catch(() => {});
        invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
        return;
      }

      // If navigating to a terminal that was part of a suspended split → restore it
      const suspended = this.suspendedSplitViews.get(wsId);
      if (suspended && (id === suspended.leftTerminalId || id === suspended.rightTerminalId)) {
        this.suspendedSplitViews.delete(wsId);
        this.setState({
          activeTerminalId: id,
          splitViews: { ...this.state.splitViews, [wsId]: suspended },
        });
        invoke('set_split_view', {
          workspaceId: wsId,
          leftTerminalId: suspended.leftTerminalId,
          rightTerminalId: suspended.rightTerminalId,
          direction: suspended.direction,
          ratio: suspended.ratio,
        }).catch(() => {});
        invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
        return;
      }
    }
    this.setState({ activeTerminalId: id });
    invoke('sync_active_terminal', { terminalId: id }).catch(() => {});
  }

  moveTerminalToWorkspace(terminalId: string, workspaceId: string) {
    const terminal = this.state.terminals.find(t => t.id === terminalId);
    let splitViews = this.state.splitViews;

    // Clear split (active or suspended) on source workspace if moved terminal was in it
    if (terminal) {
      const split = splitViews[terminal.workspaceId];
      if (split && (split.leftTerminalId === terminalId || split.rightTerminalId === terminalId)) {
        const { [terminal.workspaceId]: _, ...rest } = splitViews;
        splitViews = rest;
      }
      const suspended = this.suspendedSplitViews.get(terminal.workspaceId);
      if (suspended && (suspended.leftTerminalId === terminalId || suspended.rightTerminalId === terminalId)) {
        this.suspendedSplitViews.delete(terminal.workspaceId);
      }
    }

    this.setState({
      terminals: this.state.terminals.map(t =>
        t.id === terminalId ? { ...t, workspaceId } : t
      ),
      splitViews,
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
  }

  // Split view operations
  setSplitView(
    workspaceId: string,
    leftTerminalId: string,
    rightTerminalId: string,
    direction: 'horizontal' | 'vertical',
    ratio = 0.5,
  ) {
    this.setState({
      splitViews: {
        ...this.state.splitViews,
        [workspaceId]: { leftTerminalId, rightTerminalId, direction, ratio },
      },
    });
  }

  clearSplitView(workspaceId: string) {
    this.suspendedSplitViews.delete(workspaceId);
    const { [workspaceId]: _, ...rest } = this.state.splitViews;
    this.setState({ splitViews: rest });
  }

  getSplitView(workspaceId: string): SplitView | null {
    return this.state.splitViews[workspaceId] ?? null;
  }

  updateSplitRatio(workspaceId: string, ratio: number) {
    const split = this.state.splitViews[workspaceId];
    if (!split) return;
    this.setState({
      splitViews: {
        ...this.state.splitViews,
        [workspaceId]: { ...split, ratio },
      },
    });
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
    if (this.windowMode === 'mcp') {
      return this.state.workspaces.filter(w => w.name === 'Agent');
    }
    return this.state.workspaces.filter(w => w.name !== 'Agent');
  }
}

export const store = new Store();
