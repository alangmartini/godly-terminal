export interface Terminal {
  id: string;
  workspaceId: string;
  name: string;
  processName: string;
  order: number;
}

export type ShellType =
  | { type: 'windows' }
  | { type: 'wsl'; distribution?: string };

export interface Workspace {
  id: string;
  name: string;
  folderPath: string;
  tabOrder: string[];
  shellType: ShellType;
  worktreeMode: boolean;
  claudeCodeMode: boolean;
}

export interface AppState {
  workspaces: Workspace[];
  terminals: Terminal[];
  activeWorkspaceId: string | null;
  activeTerminalId: string | null;
}

type Listener = () => void;

class Store {
  private state: AppState = {
    workspaces: [],
    terminals: [],
    activeWorkspaceId: null,
    activeTerminalId: null,
  };

  private listeners: Set<Listener> = new Set();
  private lastActiveTerminalByWorkspace: Map<string, string> = new Map();

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
    };
    this.lastActiveTerminalByWorkspace.clear();
    this.notify();
  }

  subscribe(listener: Listener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify() {
    this.listeners.forEach(listener => listener());
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
    this.setState({
      workspaces: this.state.workspaces.filter(w => w.id !== id),
      terminals: this.state.terminals.filter(t => t.workspaceId !== id),
      activeWorkspaceId: this.state.activeWorkspaceId === id
        ? (this.state.workspaces[0]?.id ?? null)
        : this.state.activeWorkspaceId,
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

    this.setState({
      terminals: remainingTerminals,
      activeTerminalId: newActiveId,
    });
  }

  setActiveTerminal(id: string | null) {
    if (id && this.state.activeWorkspaceId) {
      this.lastActiveTerminalByWorkspace.set(this.state.activeWorkspaceId, id);
    }
    this.setState({ activeTerminalId: id });
  }

  moveTerminalToWorkspace(terminalId: string, workspaceId: string) {
    this.setState({
      terminals: this.state.terminals.map(t =>
        t.id === terminalId ? { ...t, workspaceId } : t
      ),
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

  getWorkspaceTerminals(workspaceId: string): Terminal[] {
    return this.state.terminals
      .filter(t => t.workspaceId === workspaceId)
      .sort((a, b) => a.order - b.order);
  }

  getTerminalCount(workspaceId: string): number {
    return this.state.terminals.filter(t => t.workspaceId === workspaceId).length;
  }
}

export const store = new Store();
