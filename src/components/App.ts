import { store } from '../state/store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import { WorkspaceSidebar } from './WorkspaceSidebar';
import { TabBar } from './TabBar';
import { TerminalPane } from './TerminalPane';

export class App {
  private container: HTMLElement;
  private sidebar: WorkspaceSidebar;
  private tabBar: TabBar;
  private terminalContainer: HTMLElement;
  private terminalPanes: Map<string, TerminalPane> = new Map();

  constructor(container: HTMLElement) {
    this.container = container;

    // Create sidebar
    this.sidebar = new WorkspaceSidebar();
    this.sidebar.setOnTabDrop((workspaceId, terminalId) => {
      this.handleMoveTab(workspaceId, terminalId);
    });

    // Create main content area
    const mainContent = document.createElement('div');
    mainContent.className = 'main-content';

    // Create tab bar
    this.tabBar = new TabBar();

    // Create terminal container
    this.terminalContainer = document.createElement('div');
    this.terminalContainer.className = 'terminal-container';

    // Mount components
    this.sidebar.mount(this.container);
    this.tabBar.mount(mainContent);
    mainContent.appendChild(this.terminalContainer);
    this.container.appendChild(mainContent);

    // Subscribe to state changes
    store.subscribe(() => this.handleStateChange());

    // Setup keyboard shortcuts
    this.setupKeyboardShortcuts();
  }

  private handleStateChange() {
    const state = store.getState();

    // Create panes for new terminals
    state.terminals.forEach((terminal) => {
      if (!this.terminalPanes.has(terminal.id)) {
        const pane = new TerminalPane(terminal.id);
        pane.mount(this.terminalContainer);
        this.terminalPanes.set(terminal.id, pane);
      }
    });

    // Remove panes for deleted terminals
    const terminalIds = new Set(state.terminals.map((t) => t.id));
    this.terminalPanes.forEach((pane, id) => {
      if (!terminalIds.has(id)) {
        pane.destroy();
        this.terminalPanes.delete(id);
      }
    });

    // Update active state
    this.terminalPanes.forEach((pane, id) => {
      const terminal = state.terminals.find((t) => t.id === id);
      const isVisible =
        terminal?.workspaceId === state.activeWorkspaceId &&
        id === state.activeTerminalId;
      pane.setActive(isVisible);
    });

    // Show empty state if no terminals
    const workspaceTerminals = store.getWorkspaceTerminals(
      state.activeWorkspaceId || ''
    );
    this.updateEmptyState(workspaceTerminals.length === 0);
  }

  private updateEmptyState(isEmpty: boolean) {
    let emptyState = this.terminalContainer.querySelector('.empty-state');

    if (isEmpty && !emptyState) {
      emptyState = document.createElement('div');
      emptyState.className = 'empty-state';
      emptyState.innerHTML = `
        <div class="empty-state-icon">></div>
        <div class="empty-state-text">Press Ctrl+T to create a new terminal</div>
      `;
      this.terminalContainer.appendChild(emptyState);
    } else if (!isEmpty && emptyState) {
      emptyState.remove();
    }
  }

  private async handleMoveTab(workspaceId: string, terminalId: string) {
    const terminal = store.getState().terminals.find((t) => t.id === terminalId);
    if (terminal && terminal.workspaceId !== workspaceId) {
      await workspaceService.moveTabToWorkspace(terminalId, workspaceId);
    }
  }

  private setupKeyboardShortcuts() {
    document.addEventListener('keydown', async (e) => {
      const state = store.getState();

      // Ctrl+T: New terminal
      if (e.ctrlKey && e.key === 't') {
        e.preventDefault();
        if (state.activeWorkspaceId) {
          const terminalId = await terminalService.createTerminal(
            state.activeWorkspaceId
          );
          store.addTerminal({
            id: terminalId,
            workspaceId: state.activeWorkspaceId,
            name: 'Terminal',
            processName: 'powershell',
            order: 0,
          });
        }
      }

      // Ctrl+W: Close terminal
      if (e.ctrlKey && e.key === 'w') {
        e.preventDefault();
        if (state.activeTerminalId) {
          await terminalService.closeTerminal(state.activeTerminalId);
          store.removeTerminal(state.activeTerminalId);
        }
      }

      // Ctrl+Tab: Next terminal
      if (e.ctrlKey && e.key === 'Tab') {
        e.preventDefault();
        const terminals = store.getWorkspaceTerminals(
          state.activeWorkspaceId || ''
        );
        if (terminals.length > 1 && state.activeTerminalId) {
          const currentIndex = terminals.findIndex(
            (t) => t.id === state.activeTerminalId
          );
          const nextIndex = e.shiftKey
            ? (currentIndex - 1 + terminals.length) % terminals.length
            : (currentIndex + 1) % terminals.length;
          store.setActiveTerminal(terminals[nextIndex].id);
        }
      }
    });
  }

  async init() {
    // Initialize terminal service
    await terminalService.init();

    // Load persisted state
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      const layout = await invoke<{
        workspaces: Array<{
          id: string;
          name: string;
          folder_path: string;
          tab_order: string[];
        }>;
        terminals: Array<{
          id: string;
          workspace_id: string;
          name: string;
        }>;
        active_workspace_id: string | null;
      }>('load_layout');

      if (layout.workspaces.length > 0) {
        // Restore workspaces
        layout.workspaces.forEach((w) => {
          store.addWorkspace({
            id: w.id,
            name: w.name,
            folderPath: w.folder_path,
            tabOrder: w.tab_order,
          });
        });

        // Set active workspace
        store.setActiveWorkspace(
          layout.active_workspace_id || layout.workspaces[0].id
        );

        // Recreate terminals
        for (const t of layout.terminals) {
          const terminalId = await terminalService.createTerminal(t.workspace_id);
          store.addTerminal({
            id: terminalId,
            workspaceId: t.workspace_id,
            name: t.name,
            processName: 'powershell',
            order: 0,
          });
        }
      } else {
        // Create default workspace
        await this.createDefaultWorkspace();
      }
    } catch {
      // If no saved layout, create default workspace
      await this.createDefaultWorkspace();
    }
  }

  private async createDefaultWorkspace() {
    // Get user home directory via Tauri
    const { homeDir } = await import('@tauri-apps/api/path');
    const homePath = await homeDir().catch(() => 'C:\\');

    const workspaceId = await workspaceService.createWorkspace(
      'Default',
      homePath
    );
    store.setActiveWorkspace(workspaceId);

    // Create initial terminal
    const terminalId = await terminalService.createTerminal(workspaceId);
    store.addTerminal({
      id: terminalId,
      workspaceId,
      name: 'Terminal',
      processName: 'powershell',
      order: 0,
    });
  }
}
