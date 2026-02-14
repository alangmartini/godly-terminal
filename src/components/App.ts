import { store, ShellType } from '../state/store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import { keybindingStore, formatChord } from '../state/keybinding-store';
import { notificationStore } from '../state/notification-store';
import { playNotificationSound } from '../services/notification-sound';
import { quotePath } from '../utils/quote-path';
import { WorkspaceSidebar } from './WorkspaceSidebar';
import { TabBar, getDisplayName } from './TabBar';
import { TerminalPane } from './TerminalPane';
import { ToastContainer } from './ToastContainer';

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
  private toastContainer: ToastContainer;
  private terminalPanes: Map<string, TerminalPane> = new Map();
  private restoredTerminalIds: Set<string> = new Set();
  /** Terminal IDs that were reattached to live daemon sessions (no scrollback load needed) */
  private reattachedTerminalIds: Set<string> = new Set();
  private splitDivider: HTMLElement | null = null;
  private splitDropOverlay: HTMLElement | null = null;

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
    this.tabBar.setOnSplit((terminalId, direction) => {
      this.handleSplitRequest(terminalId, direction);
    });
    this.tabBar.setOnUnsplit(() => {
      this.handleUnsplitRequest();
    });

    // Create terminal container
    this.terminalContainer = document.createElement('div');
    this.terminalContainer.className = 'terminal-container';

    // Create toast container for in-app notifications
    this.toastContainer = new ToastContainer();

    // Mount components
    if (store.windowMode === 'main') {
      this.sidebar.mount(this.container);
    }
    this.tabBar.mount(mainContent);
    mainContent.appendChild(this.terminalContainer);
    this.container.appendChild(mainContent);
    this.toastContainer.mount(document.body);

    // Subscribe to state changes
    store.subscribe(() => this.handleStateChange());

    // Setup keyboard shortcuts
    this.setupKeyboardShortcuts();

    // Setup split drop zones for tab drag-drop
    this.setupSplitDropZone();
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

    // Update active state (split-aware)
    const split = state.activeWorkspaceId
      ? store.getSplitView(state.activeWorkspaceId)
      : null;

    if (split) {
      // Split mode: show two panes
      this.terminalContainer.classList.remove('split-horizontal', 'split-vertical');
      this.terminalContainer.classList.add(
        split.direction === 'horizontal' ? 'split-horizontal' : 'split-vertical'
      );

      const leftPane = this.terminalPanes.get(split.leftTerminalId);
      const rightPane = this.terminalPanes.get(split.rightTerminalId);

      // Ensure divider exists
      this.ensureSplitDivider(split.direction);

      // Reorder DOM: left pane, divider, right pane
      if (leftPane) {
        this.terminalContainer.insertBefore(
          leftPane.getContainer(),
          this.terminalContainer.firstChild
        );
      }
      if (this.splitDivider && leftPane) {
        leftPane.getContainer().after(this.splitDivider);
      }
      if (rightPane && this.splitDivider) {
        this.splitDivider.after(rightPane.getContainer());
      }

      // Set flex-basis based on ratio
      const leftBasis = `calc(${split.ratio * 100}% - 2px)`;
      const rightBasis = `calc(${(1 - split.ratio) * 100}% - 2px)`;

      this.terminalPanes.forEach((pane, id) => {
        const terminal = state.terminals.find((t) => t.id === id);
        if (terminal?.workspaceId !== state.activeWorkspaceId) {
          pane.setActive(false);
          return;
        }

        if (id === split.leftTerminalId) {
          pane.getContainer().style.flexBasis = leftBasis;
          pane.setSplitVisible(true, state.activeTerminalId === id);
        } else if (id === split.rightTerminalId) {
          pane.getContainer().style.flexBasis = rightBasis;
          pane.setSplitVisible(true, state.activeTerminalId === id);
        } else {
          pane.setActive(false);
        }
      });
    } else {
      // Single pane mode
      this.terminalContainer.classList.remove('split-horizontal', 'split-vertical');
      this.removeSplitDivider();

      this.terminalPanes.forEach((pane, id) => {
        pane.getContainer().style.flexBasis = '';
        const terminal = state.terminals.find((t) => t.id === id);
        const isVisible =
          terminal?.workspaceId === state.activeWorkspaceId &&
          id === state.activeTerminalId;
        pane.setActive(isVisible);
      });
    }

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

        case 'split.focusOtherPane': {
          e.preventDefault();
          if (state.activeWorkspaceId && state.activeTerminalId) {
            const activeSplit = store.getSplitView(state.activeWorkspaceId);
            if (activeSplit) {
              const otherId = state.activeTerminalId === activeSplit.leftTerminalId
                ? activeSplit.rightTerminalId
                : activeSplit.leftTerminalId;
              store.setActiveTerminal(otherId);
            }
          }
          break;
        }

        case 'workspace.toggleWorktreeMode': {
          e.preventDefault();
          if (state.activeWorkspaceId) {
            const workspace = state.workspaces.find(w => w.id === state.activeWorkspaceId);
            if (workspace) {
              if (!workspace.worktreeMode) {
                const isGit = await workspaceService.isGitRepo(workspace.folderPath).catch(() => false);
                if (!isGit) {
                  console.warn('[App] Cannot enable worktree mode: not a git repository');
                  break;
                }
              }
              await workspaceService.toggleWorktreeMode(workspace.id, !workspace.worktreeMode);
            }
          }
          break;
        }

        case 'workspace.toggleClaudeCodeMode': {
          e.preventDefault();
          if (state.activeWorkspaceId) {
            const workspace = state.workspaces.find(w => w.id === state.activeWorkspaceId);
            if (workspace) {
              await workspaceService.toggleClaudeCodeMode(workspace.id, !workspace.claudeCodeMode);
            }
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
    // (only main window handles scrollback saves)
    if (store.windowMode === 'main') {
      await this.setupScrollbackSaveListener();
    }

    // Listen for file drag-drop events (paste file names into terminal)
    this.setupDragDropListener();

    // Listen for MCP-triggered UI events
    await this.setupMcpEventListeners();

    // MCP window: no layout to restore, just wait for events
    if (store.windowMode === 'mcp') {
      console.log('[App] MCP mode — waiting for agent terminals');
      return;
    }

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
        split_views?: Record<string, {
          left_terminal_id: string;
          right_terminal_id: string;
          direction: string;
          ratio: number;
        }>;
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
        // Filter out dead sessions (PTY exited but session still in daemon HashMap)
        const liveSessionIds = new Set(
          liveSessions.filter((s) => s.running).map((s) => s.id)
        );
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
        // Restore split views
        if (layout.split_views) {
          const terminalIds = new Set(store.getState().terminals.map(t => t.id));
          for (const [wsId, sv] of Object.entries(layout.split_views)) {
            if (terminalIds.has(sv.left_terminal_id) && terminalIds.has(sv.right_terminal_id)) {
              const dir = sv.direction === 'vertical' ? 'vertical' : 'horizontal';
              store.setSplitView(wsId, sv.left_terminal_id, sv.right_terminal_id, dir, sv.ratio);
              await this.syncSplitToBackend(wsId, {
                leftTerminalId: sv.left_terminal_id,
                rightTerminalId: sv.right_terminal_id,
                direction: dir,
                ratio: sv.ratio,
              });
            }
          }
          console.log('[App] Split views restored:', Object.keys(layout.split_views).length);
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

  private ensureSplitDivider(direction: string) {
    if (this.splitDivider) {
      // Update direction class if needed
      this.splitDivider.classList.remove('horizontal', 'vertical');
      this.splitDivider.classList.add(direction);
      return;
    }

    this.splitDivider = document.createElement('div');
    this.splitDivider.className = `split-divider ${direction}`;
    this.terminalContainer.appendChild(this.splitDivider);

    // Drag to resize
    this.splitDivider.addEventListener('mousedown', (e) => {
      e.preventDefault();
      const state = store.getState();
      if (!state.activeWorkspaceId) return;
      const split = store.getSplitView(state.activeWorkspaceId);
      if (!split) return;

      const isHorizontal = split.direction === 'horizontal';
      const rect = this.terminalContainer.getBoundingClientRect();

      const onMouseMove = (moveEvent: MouseEvent) => {
        let ratio: number;
        if (isHorizontal) {
          ratio = (moveEvent.clientX - rect.left) / rect.width;
        } else {
          ratio = (moveEvent.clientY - rect.top) / rect.height;
        }
        ratio = Math.max(0.15, Math.min(0.85, ratio));
        store.updateSplitRatio(state.activeWorkspaceId!, ratio);
      };

      const onMouseUp = async () => {
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
        // Sync final ratio to backend
        const currentSplit = store.getSplitView(state.activeWorkspaceId!);
        if (currentSplit) {
          await this.syncSplitToBackend(state.activeWorkspaceId!, currentSplit);
        }
      };

      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
    });
  }

  private removeSplitDivider() {
    if (this.splitDivider) {
      this.splitDivider.remove();
      this.splitDivider = null;
    }
  }

  private setupSplitDropZone() {
    // Create drop overlay element
    this.splitDropOverlay = document.createElement('div');
    this.splitDropOverlay.className = 'split-drop-overlay';
    this.terminalContainer.appendChild(this.splitDropOverlay);

    this.terminalContainer.addEventListener('dragover', (e) => {
      // Only handle tab drags (text/plain contains terminal ID)
      if (!e.dataTransfer?.types.includes('text/plain')) return;

      e.preventDefault();
      e.dataTransfer!.dropEffect = 'move';

      const rect = this.terminalContainer.getBoundingClientRect();
      const x = (e.clientX - rect.left) / rect.width;
      const y = (e.clientY - rect.top) / rect.height;

      const overlay = this.splitDropOverlay!;

      // Determine drop zone
      if (x > 0.7) {
        // Right edge
        overlay.style.left = '50%';
        overlay.style.top = '0';
        overlay.style.width = '50%';
        overlay.style.height = '100%';
        overlay.textContent = 'Split Right';
        overlay.classList.add('visible');
        overlay.dataset.zone = 'right';
      } else if (x < 0.3) {
        // Left edge
        overlay.style.left = '0';
        overlay.style.top = '0';
        overlay.style.width = '50%';
        overlay.style.height = '100%';
        overlay.textContent = 'Split Left';
        overlay.classList.add('visible');
        overlay.dataset.zone = 'left';
      } else if (y > 0.7) {
        // Bottom edge
        overlay.style.left = '0';
        overlay.style.top = '50%';
        overlay.style.width = '100%';
        overlay.style.height = '50%';
        overlay.textContent = 'Split Down';
        overlay.classList.add('visible');
        overlay.dataset.zone = 'bottom';
      } else if (y < 0.3) {
        // Top edge
        overlay.style.left = '0';
        overlay.style.top = '0';
        overlay.style.width = '100%';
        overlay.style.height = '50%';
        overlay.textContent = 'Split Up';
        overlay.classList.add('visible');
        overlay.dataset.zone = 'top';
      } else {
        // Center: no split indicator
        overlay.classList.remove('visible');
        overlay.dataset.zone = '';
      }
    });

    this.terminalContainer.addEventListener('dragleave', (e) => {
      // Only hide if leaving the container entirely
      const related = e.relatedTarget as HTMLElement | null;
      if (!related || !this.terminalContainer.contains(related)) {
        this.splitDropOverlay?.classList.remove('visible');
      }
    });

    this.terminalContainer.addEventListener('drop', async (e) => {
      this.splitDropOverlay?.classList.remove('visible');

      const droppedTerminalId = e.dataTransfer?.getData('text/plain');
      const zone = this.splitDropOverlay?.dataset.zone;
      if (!droppedTerminalId || !zone) return;

      const state = store.getState();
      if (!state.activeWorkspaceId || !state.activeTerminalId) return;

      // Don't split with the same terminal
      if (droppedTerminalId === state.activeTerminalId) return;

      // Verify the dropped terminal belongs to this workspace
      const droppedTerminal = state.terminals.find(t => t.id === droppedTerminalId);
      if (!droppedTerminal || droppedTerminal.workspaceId !== state.activeWorkspaceId) return;

      // Determine direction and pane assignment
      let direction: 'horizontal' | 'vertical';
      let leftId: string;
      let rightId: string;

      const existingSplit = store.getSplitView(state.activeWorkspaceId);

      if (existingSplit) {
        // Already split: replace the pane closest to drop position
        if (zone === 'right' || zone === 'bottom') {
          leftId = existingSplit.leftTerminalId;
          rightId = droppedTerminalId;
          direction = existingSplit.direction;
        } else {
          leftId = droppedTerminalId;
          rightId = existingSplit.rightTerminalId;
          direction = existingSplit.direction;
        }
      } else {
        // New split
        if (zone === 'right') {
          direction = 'horizontal';
          leftId = state.activeTerminalId;
          rightId = droppedTerminalId;
        } else if (zone === 'left') {
          direction = 'horizontal';
          leftId = droppedTerminalId;
          rightId = state.activeTerminalId;
        } else if (zone === 'bottom') {
          direction = 'vertical';
          leftId = state.activeTerminalId;
          rightId = droppedTerminalId;
        } else {
          // top
          direction = 'vertical';
          leftId = droppedTerminalId;
          rightId = state.activeTerminalId;
        }
      }

      store.setSplitView(state.activeWorkspaceId, leftId, rightId, direction);
      const split = store.getSplitView(state.activeWorkspaceId)!;
      await this.syncSplitToBackend(state.activeWorkspaceId, split);
    });
  }

  private async syncSplitToBackend(workspaceId: string, split: { leftTerminalId: string; rightTerminalId: string; direction: string; ratio: number }) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('set_split_view', {
        workspaceId,
        leftTerminalId: split.leftTerminalId,
        rightTerminalId: split.rightTerminalId,
        direction: split.direction,
        ratio: split.ratio,
      });
    } catch (error) {
      console.error('[App] Failed to sync split view to backend:', error);
    }
  }

  private async handleSplitRequest(terminalId: string, direction: 'horizontal' | 'vertical') {
    const state = store.getState();
    if (!state.activeWorkspaceId || !state.activeTerminalId) return;
    if (terminalId === state.activeTerminalId) return;

    const leftId = state.activeTerminalId;
    const rightId = terminalId;

    store.setSplitView(state.activeWorkspaceId, leftId, rightId, direction);
    const split = store.getSplitView(state.activeWorkspaceId)!;
    await this.syncSplitToBackend(state.activeWorkspaceId, split);
  }

  private async handleUnsplitRequest() {
    const state = store.getState();
    if (!state.activeWorkspaceId) return;
    store.clearSplitView(state.activeWorkspaceId);
    await this.clearSplitFromBackend(state.activeWorkspaceId);
  }

  private async clearSplitFromBackend(workspaceId: string) {
    try {
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('clear_split_view', { workspaceId });
    } catch (error) {
      console.error('[App] Failed to clear split view from backend:', error);
    }
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

  private setupDragDropListener() {
    // File drop handling via HTML5 events.
    // We use dragDropEnabled: false in tauri.conf.json because Tauri's native
    // IDropTarget intercepts ALL drag operations (including internal HTML5 DnD
    // for tab/workspace reordering) and returns DROPEFFECT_NONE for non-file
    // drags, causing Windows to show a forbidden cursor and preventing drops.
    // With dragDropEnabled: false, WebView2 uses its default drop handling
    // which supports both internal DnD and external file drops via HTML5 events.
    // Limitation: File.path is not available in WebView2, so we paste filenames
    // instead of full paths when files are dropped from Explorer.
    let dragCounter = 0;

    this.terminalContainer.addEventListener('dragenter', (e) => {
      // Only react to external file drags, not internal tab drags
      if (!e.dataTransfer?.types.includes('Files')) return;
      dragCounter++;
      this.terminalContainer.classList.add('drag-file-over');
    });

    this.terminalContainer.addEventListener('dragover', (e) => {
      // Must preventDefault on dragover to allow the drop event to fire.
      // Without this, the browser refuses the drop and our cleanup in the
      // drop handler never runs, leaving the overlay stuck.
      if (!e.dataTransfer?.types.includes('Files')) return;
      e.preventDefault();
      e.dataTransfer!.dropEffect = 'copy';
    });

    this.terminalContainer.addEventListener('dragleave', (e) => {
      if (!e.dataTransfer?.types.includes('Files')) return;
      dragCounter--;
      if (dragCounter <= 0) {
        dragCounter = 0;
        this.terminalContainer.classList.remove('drag-file-over');
      }
    });

    this.terminalContainer.addEventListener('drop', (e) => {
      if (!e.dataTransfer?.types.includes('Files')) return;
      e.preventDefault();
      dragCounter = 0;
      this.terminalContainer.classList.remove('drag-file-over');

      const state = store.getState();
      if (!state.activeTerminalId || !e.dataTransfer.files.length) return;

      const names = Array.from(e.dataTransfer.files).map(f => f.name);
      const text = names.map(quotePath).join(' ');
      terminalService.writeToTerminal(state.activeTerminalId, text);
    });

    // Safety net: if a file drag ends without a clean drop (e.g. dropped
    // outside the window, or Escape pressed), clear the overlay.
    document.addEventListener('dragend', () => {
      dragCounter = 0;
      this.terminalContainer.classList.remove('drag-file-over');
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
    await listen<{ terminal_id: string; workspace_id: string }>('mcp-terminal-created', async (event) => {
      const { terminal_id: terminalId, workspace_id: workspaceId } = event.payload;
      console.log('[App] MCP terminal created:', terminalId, 'in workspace:', workspaceId);

      // In main window: ignore Agent workspace terminals (they belong to the MCP window)
      // In MCP window: only handle Agent workspace terminals
      if (store.windowMode === 'main') {
        // Ensure the Agent workspace exists in main window store (hidden from sidebar)
        const state = store.getState();
        if (!state.workspaces.find(w => w.id === workspaceId)) {
          // Add the Agent workspace to the store (sidebar will filter it out)
          store.addWorkspace({
            id: workspaceId,
            name: 'Agent',
            folderPath: '',
            tabOrder: [],
            shellType: { type: 'windows' },
            worktreeMode: false,
            claudeCodeMode: false,
          });
        }
        // Don't add the terminal to the main window — MCP window handles it
        return;
      }

      // MCP window: add the terminal
      const state = store.getState();
      const existing = state.terminals.find((t) => t.id === terminalId);
      if (!existing) {
        // Ensure workspace exists in MCP window store
        if (!state.workspaces.find(w => w.id === workspaceId)) {
          store.addWorkspace({
            id: workspaceId,
            name: 'Agent',
            folderPath: '',
            tabOrder: [],
            shellType: { type: 'windows' },
            worktreeMode: false,
            claudeCodeMode: false,
          });
          store.setActiveWorkspace(workspaceId);
        }
        store.addTerminal({
          id: terminalId,
          workspaceId,
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

        // Show in-app toast (always, regardless of focus)
        const terminal = state.terminals.find(t => t.id === terminal_id);
        const toastTitle = terminal ? getDisplayName(terminal) : 'Terminal';
        this.toastContainer.show(toastTitle, message || 'New notification', terminal_id);

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
              const title = terminal ? getDisplayName(terminal) : 'Godly Terminal';
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
