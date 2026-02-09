import { store, ShellType } from '../state/store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import { keybindingStore, formatChord } from '../state/keybinding-store';
import { notificationStore } from '../state/notification-store';
import { playNotificationSound } from '../services/notification-sound';
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
  /** Terminal IDs that were reattached to live daemon sessions (no scrollback load needed) */
  private reattachedTerminalIds: Set<string> = new Set();

  constructor(container: HTMLElement) {
    this.container = container;

    // Expose store for E2E test access
    (window as any).__store = store;

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

    // Clear notification badge for the now-active terminal
    if (state.activeTerminalId) {
      notificationStore.clearBadge(state.activeTerminalId);
    }

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
      const icon = document.createElement('div');
      icon.className = 'empty-state-icon';
      icon.textContent = '>';
      const text = document.createElement('div');
      text.className = 'empty-state-text';
      text.textContent = `Press ${formatChord(keybindingStore.getBinding('tabs.newTerminal'))} to create a new terminal`;
      emptyState.appendChild(icon);
      emptyState.appendChild(text);
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

      // ── Hardcoded shortcuts (not customisable) ───────────────────

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

      // Ctrl+, : Open settings dialog
      if (e.ctrlKey && !e.shiftKey && e.key === ',') {
        e.preventDefault();
        const { showSettingsDialog } = await import('./SettingsDialog');
        await showSettingsDialog();
        return;
      }

      // ── Dynamic shortcuts (customisable via settings) ────────────

      const action = keybindingStore.matchAction(e);
      if (!action) return;

      switch (action) {
        case 'tabs.newTerminal': {
          e.preventDefault();
          if (state.activeWorkspaceId) {
            const workspace = state.workspaces.find(w => w.id === state.activeWorkspaceId);
            let worktreeName: string | undefined;

            if (workspace?.worktreeMode) {
              const { showWorktreeNamePrompt } = await import('./dialogs');
              const name = await showWorktreeNamePrompt();
              if (name === null) return;
              worktreeName = name || undefined;
            }

            const result = await terminalService.createTerminal(
              state.activeWorkspaceId,
              { worktreeName }
            );
            store.addTerminal({
              id: result.id,
              workspaceId: state.activeWorkspaceId,
              name: result.worktree_branch ?? 'Terminal',
              processName: 'powershell',
              order: 0,
            });

            if (workspace?.claudeCodeMode) {
              setTimeout(() => {
                terminalService.writeToTerminal(result.id, 'claude -dangerously-skip-permissions\r');
              }, 500);
            }
          }
          break;
        }

        case 'tabs.closeTerminal': {
          e.preventDefault();
          if (state.activeTerminalId) {
            await terminalService.closeTerminal(state.activeTerminalId);
            store.removeTerminal(state.activeTerminalId);
          }
          break;
        }

        case 'tabs.nextTab': {
          e.preventDefault();
          const terminals = store.getWorkspaceTerminals(
            state.activeWorkspaceId || ''
          );
          if (terminals.length > 1 && state.activeTerminalId) {
            const currentIndex = terminals.findIndex(
              (t) => t.id === state.activeTerminalId
            );
            const nextIndex = (currentIndex + 1) % terminals.length;
            store.setActiveTerminal(terminals[nextIndex].id);
          }
          break;
        }

        case 'tabs.previousTab': {
          e.preventDefault();
          const terminals = store.getWorkspaceTerminals(
            state.activeWorkspaceId || ''
          );
          if (terminals.length > 1 && state.activeTerminalId) {
            const currentIndex = terminals.findIndex(
              (t) => t.id === state.activeTerminalId
            );
            const nextIndex = (currentIndex - 1 + terminals.length) % terminals.length;
            store.setActiveTerminal(terminals[nextIndex].id);
          }
          break;
        }
      }
    });
  }

  async init() {
    // Initialize terminal service
    await terminalService.init();

    // Listen for scrollback save requests from backend (on window close)
    await this.setupScrollbackSaveListener();

    // Listen for MCP-triggered UI events
    await this.setupMcpEventListeners();

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
          worktree_mode?: boolean;
          claude_code_mode?: boolean;
        }>;
        terminals: Array<{
          id: string;
          workspace_id: string;
          name: string;
          shell_type?: BackendShellType;
          cwd?: string | null;
          worktree_path?: string | null;
          worktree_branch?: string | null;
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
            worktreeMode: w.worktree_mode ?? false,
            claudeCodeMode: w.claude_code_mode ?? false,
          });
        });

        // Set active workspace
        const activeWsId = layout.active_workspace_id || layout.workspaces[0].id;
        console.log('[App] Setting active workspace:', activeWsId);
        store.setActiveWorkspace(activeWsId);

        // Check daemon for live sessions that can be reattached
        const liveSessions = await terminalService.reconnectSessions();
        const liveSessionIds = new Set(liveSessions.map((s) => s.id));
        console.log('[App] Live daemon sessions:', liveSessionIds.size);

        // Restore terminals: reattach if alive, or create fresh with scrollback
        console.log('[App] Restoring terminals...');
        for (const t of layout.terminals) {
          const shellType = convertShellType(t.shell_type);
          const processName =
            shellType.type === 'wsl'
              ? shellType.distribution ?? 'wsl'
              : 'powershell';

          const tabName = t.worktree_branch || t.name;

          if (liveSessionIds.has(t.id)) {
            // Session is still alive in daemon - reattach
            console.log('[App] Reattaching to live session:', t.id);
            try {
              await terminalService.attachSession(t.id, t.workspace_id, tabName);
              this.reattachedTerminalIds.add(t.id);

              store.addTerminal({
                id: t.id,
                workspaceId: t.workspace_id,
                name: tabName,
                processName,
                order: 0,
              });
              continue;
            } catch (error) {
              console.warn('[App] Failed to reattach session:', t.id, error);
              // Fall through to create fresh terminal
            }
          }

          // Session is dead or reattach failed - create fresh terminal with saved CWD
          console.log('[App] Creating fresh terminal:', t.id, 'in workspace:', t.workspace_id);
          const result = await terminalService.createTerminal(t.workspace_id, {
            cwdOverride: t.cwd ?? undefined,
            shellTypeOverride: shellType,
            idOverride: t.id,
            nameOverride: tabName,
          });

          console.log('[App] Terminal created with ID:', result.id, '(requested:', t.id, ')');

          // Mark for scrollback restoration (only for fresh terminals, not reattached ones)
          this.restoredTerminalIds.add(result.id);

          store.addTerminal({
            id: result.id,
            workspaceId: t.workspace_id,
            name: tabName,
            processName,
            order: 0,
          });
        }
        console.log('[App] Restore complete!');
      } else {
        console.log('[App] No workspaces in layout, creating default...');
        await this.createDefaultWorkspace();
      }
    } catch (error) {
      console.error('[App] Error loading layout:', error);
      (window as any).__app_init_error = String(error);
      try {
        await this.createDefaultWorkspace();
      } catch (e2) {
        console.error('[App] Error creating default workspace:', e2);
        (window as any).__app_init_error2 = String(e2);
      }
    }
  }

  private async createDefaultWorkspace() {
    // Get user home directory via Tauri
    const { homeDir } = await import('@tauri-apps/api/path');
    const homePath = await homeDir().catch(() => 'C:\\');
    console.log('[App] Home path:', homePath);

    const workspaceId = await workspaceService.createWorkspace(
      'Default',
      homePath
    );
    console.log('[App] Workspace created:', workspaceId);
    store.setActiveWorkspace(workspaceId);

    // Create initial terminal
    console.log('[App] Creating terminal in workspace:', workspaceId);
    const result = await terminalService.createTerminal(workspaceId);
    console.log('[App] Terminal created:', result.id);
    store.addTerminal({
      id: result.id,
      workspaceId,
      name: result.worktree_branch ?? 'Terminal',
      processName: 'powershell',
      order: 0,
    });
    console.log('[App] Terminal added to store');
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

  private async setupMcpEventListeners() {
    const { listen } = await import('@tauri-apps/api/event');

    // MCP: focus a terminal tab
    await listen<string>('focus-terminal', (event) => {
      const terminalId = event.payload;
      console.log('[App] MCP focus-terminal:', terminalId);
      const terminal = store.getState().terminals.find((t) => t.id === terminalId);
      if (terminal) {
        store.setActiveWorkspace(terminal.workspaceId);
        store.setActiveTerminal(terminalId);
      }
    });

    // MCP: switch workspace
    await listen<string>('switch-workspace', (event) => {
      const workspaceId = event.payload;
      console.log('[App] MCP switch-workspace:', workspaceId);
      store.setActiveWorkspace(workspaceId);
    });

    // MCP: terminal renamed
    await listen<{ terminal_id: string; name: string }>('terminal-renamed', (event) => {
      const { terminal_id, name } = event.payload;
      console.log('[App] MCP terminal-renamed:', terminal_id, name);
      store.updateTerminal(terminal_id, { name });
    });

    // MCP: terminal created by MCP handler
    await listen<string>('mcp-terminal-created', async (event) => {
      const terminalId = event.payload;
      console.log('[App] MCP terminal created:', terminalId);
      // The terminal was already added to backend state by the MCP handler.
      // Add it to the frontend store with a default name.
      // The process-changed event will update the process name later.
      const state = store.getState();
      const existing = state.terminals.find((t) => t.id === terminalId);
      if (!existing) {
        store.addTerminal({
          id: terminalId,
          workspaceId: state.activeWorkspaceId || '',
          name: 'Terminal',
          processName: 'powershell',
          order: 0,
        });
      }
    });

    // MCP: terminal closed by MCP handler
    await listen<string>('mcp-terminal-closed', (event) => {
      const terminalId = event.payload;
      console.log('[App] MCP terminal closed:', terminalId);
      store.removeTerminal(terminalId);
    });

    // MCP: terminal moved to different workspace
    await listen<{ terminal_id: string; workspace_id: string }>('mcp-terminal-moved', (event) => {
      const { terminal_id, workspace_id } = event.payload;
      console.log('[App] MCP terminal moved:', terminal_id, 'to', workspace_id);
      store.moveTerminalToWorkspace(terminal_id, workspace_id);
    });

    // MCP: notification triggered
    await listen<{ terminal_id: string; message: string | null }>('mcp-notify', async (event) => {
      const { terminal_id, message } = event.payload;
      const settings = notificationStore.getSettings();
      if (!settings.globalEnabled) return;

      const state = store.getState();
      const isActive = state.activeTerminalId === terminal_id;

      const played = notificationStore.recordNotify(terminal_id);
      if (played) {
        playNotificationSound(settings.soundPreset, settings.volume);

        // Flash taskbar icon orange
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        getCurrentWindow().requestUserAttention(2); // Informational = brief orange flash

        // Show Windows native notification if window is not focused
        if (!document.hasFocus()) {
          try {
            const { isPermissionGranted, requestPermission, sendNotification } =
              await import('@tauri-apps/plugin-notification');
            let permitted = await isPermissionGranted();
            if (!permitted) {
              const result = await requestPermission();
              permitted = result === 'granted';
            }
            if (permitted) {
              const terminal = state.terminals.find(t => t.id === terminal_id);
              const title = terminal?.name || 'Godly Terminal';
              sendNotification({
                title,
                body: message || 'New notification',
              });
            }
          } catch (e) {
            console.warn('[App] Failed to send native notification:', e);
          }
        }
      }

      // If already the active terminal, clear badge immediately
      if (isActive) {
        notificationStore.clearBadge(terminal_id);
      }
    });
  }
}
