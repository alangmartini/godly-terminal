import { store, ShellType } from '../state/store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import { WorkspaceSidebar } from './WorkspaceSidebar';
import { TabBar } from './TabBar';
import { TerminalPane } from './TerminalPane';

type BackendShellType =
  | 'windows'
  | { wsl: { distribution: string | null } };

function convertShellType(backendType?: BackendShellType): ShellType {
  if (!backendType || backendType === 'windows') {
    return { type: 'windows' };
  }
  if (typeof backendType === 'object' && 'wsl' in backendType) {
    return {
      type: 'wsl',
      distribution: backendType.wsl.distribution ?? undefined,
    };
  }
  return { type: 'windows' };
}

export class App {
  private container: HTMLElement;
  private sidebar: WorkspaceSidebar;
  private tabBar: TabBar;
  private terminalContainer: HTMLElement;
  private terminalPanes: Map<string, TerminalPane> = new Map();
  private restoredTerminalIds: Set<string> = new Set();

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

        // Load scrollback for restored terminals
        if (this.restoredTerminalIds.has(terminal.id)) {
          // Small delay to ensure terminal is mounted
          setTimeout(() => pane.loadScrollback(), 100);
          this.restoredTerminalIds.delete(terminal.id);
        }
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

      // Ctrl+Shift+S: Manual save (for debugging)
      if (e.ctrlKey && e.shiftKey && e.key === 'S') {
        e.preventDefault();
        console.log('[App] Manual save triggered...');
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          await invoke('save_layout');
          console.log('[App] Manual save complete!');
        } catch (error) {
          console.error('[App] Manual save failed:', error);
        }
        return;
      }

      // Ctrl+Shift+L: Manual load (for debugging)
      if (e.ctrlKey && e.shiftKey && e.key === 'L') {
        e.preventDefault();
        console.log('[App] Manual load triggered...');
        try {
          const { invoke } = await import('@tauri-apps/api/core');
          const layout = await invoke('load_layout');
          console.log('[App] Manual load result:', JSON.stringify(layout, null, 2));
        } catch (error) {
          console.error('[App] Manual load failed:', error);
        }
        return;
      }

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

    // Listen for scrollback save requests from backend (on window close)
    await this.setupScrollbackSaveListener();

    // Load persisted state
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      console.log('[App] Loading layout...');
      const layout = await invoke<{
        workspaces: Array<{
          id: string;
          name: string;
          folder_path: string;
          tab_order: string[];
          shell_type?: BackendShellType;
        }>;
        terminals: Array<{
          id: string;
          workspace_id: string;
          name: string;
          shell_type?: BackendShellType;
          cwd?: string | null;
        }>;
        active_workspace_id: string | null;
      }>('load_layout');

      console.log('[App] Layout loaded:', JSON.stringify(layout, null, 2));
      console.log('[App] Workspaces count:', layout.workspaces.length);
      console.log('[App] Terminals count:', layout.terminals.length);

      if (layout.workspaces.length > 0) {
        console.log('[App] Restoring workspaces...');
        // Restore workspaces
        layout.workspaces.forEach((w) => {
          console.log('[App] Adding workspace:', w.id, w.name);
          store.addWorkspace({
            id: w.id,
            name: w.name,
            folderPath: w.folder_path,
            tabOrder: w.tab_order,
            shellType: convertShellType(w.shell_type),
          });
        });

        // Set active workspace
        const activeWsId = layout.active_workspace_id || layout.workspaces[0].id;
        console.log('[App] Setting active workspace:', activeWsId);
        store.setActiveWorkspace(activeWsId);

        // Recreate terminals with saved CWD, shell type, and original ID
        console.log('[App] Restoring terminals...');
        for (const t of layout.terminals) {
          console.log('[App] Creating terminal:', t.id, 'in workspace:', t.workspace_id);
          const shellType = convertShellType(t.shell_type);
          const terminalId = await terminalService.createTerminal(t.workspace_id, {
            cwdOverride: t.cwd ?? undefined,
            shellTypeOverride: shellType,
            idOverride: t.id, // Preserve original ID for scrollback lookup
          });

          console.log('[App] Terminal created with ID:', terminalId, '(requested:', t.id, ')');

          // Mark this terminal for scrollback restoration
          this.restoredTerminalIds.add(terminalId);

          // Determine process name from shell type
          const processName =
            shellType.type === 'wsl'
              ? shellType.distribution ?? 'wsl'
              : 'powershell';

          store.addTerminal({
            id: terminalId,
            workspaceId: t.workspace_id,
            name: t.name,
            processName,
            order: 0,
          });
        }
        console.log('[App] Restore complete!');
      } else {
        console.log('[App] No workspaces in layout, creating default...');
        // Create default workspace
        await this.createDefaultWorkspace();
      }
    } catch (error) {
      // If no saved layout, create default workspace
      console.error('[App] Error loading layout:', error);
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

  private async setupScrollbackSaveListener() {
    const { listen } = await import('@tauri-apps/api/event');
    await listen('request-scrollback-save', async () => {
      console.log('[App] Saving all scrollbacks before exit...');
      const saves = Array.from(this.terminalPanes.values()).map((pane) =>
        pane.saveScrollback()
      );
      await Promise.all(saves);
      console.log('[App] All scrollbacks saved, signaling completion...');

      // Signal completion to backend
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('scrollback_save_complete');
    });
  }
}
