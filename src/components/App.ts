import { store } from '../state/store';
import { terminalSettingsStore } from '../state/terminal-settings-store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import { keybindingStore, formatChord } from '../state/keybinding-store';
import { notificationStore } from '../state/notification-store';
import { playNotificationSound } from '../services/notification-sound';
import { getPluginRegistry } from '../plugins/index';
import { IdleNotificationService } from '../services/idle-notification-service';
import { quotePath } from '../utils/quote-path';
import { perfTracer } from '../utils/PerfTracer';
import {
  shellTypeToProcessName,
  buildNotificationTitle,
  isWorkspaceNotificationSuppressed,
} from '../utils/shell-type-utils';
import { setupKeyboardShortcuts } from '../controllers/keyboard-controller';
import { restoreLayout, syncSplitToBackend } from '../controllers/reconnection-controller';
import { handleVoiceToggle } from '../controllers/voice-controller';
import { WorkspaceSidebar } from './WorkspaceSidebar';
import { TabBar } from './TabBar';
import { TerminalPane } from './TerminalPane';
import { FigmaPane } from './FigmaPane';
import { ToastContainer } from './ToastContainer';
import { PerfOverlay } from './PerfOverlay';
import { onDragMove, onDragDrop } from '../state/drag-state';
import { SplitContainer } from './SplitContainer';
import { RecencySwitcher } from './RecencySwitcher';
import { terminalIds, fromLegacySplitView, swapTerminals } from '../state/split-types';

// Re-export for backward compatibility (used by test files)
export { buildNotificationTitle } from '../utils/shell-type-utils';

export class App {
  private container: HTMLElement;
  private sidebar: WorkspaceSidebar;
  private tabBar: TabBar;
  private terminalContainer: HTMLElement;
  private toastContainer: ToastContainer;
  private terminalPanes: Map<string, TerminalPane | FigmaPane> = new Map();
  private restoredTerminalIds: Set<string> = new Set();
  /** Terminal IDs that were reattached to live daemon sessions (no scrollback load needed) */
  private reattachedTerminalIds: Set<string> = new Set();
  private splitDivider: HTMLElement | null = null;
  private splitDropOverlay: HTMLElement | null = null;
  private splitContainer: SplitContainer | null = null;
  private splitContainerWorkspaceId: string | null = null;
  private perfOverlay: PerfOverlay | null = null;
  /** Tracks which pane is currently zoomed (null = no zoom active). */
  private zoomedPaneId: string | null = null;
  /** Stores the split ratio before zoom, so it can be restored on unzoom. */
  private preZoomRatio: number | null = null;
  private recencySwitcher: RecencySwitcher;
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
    this.sidebar.mount(this.container);
    this.tabBar.mount(mainContent);
    mainContent.appendChild(this.terminalContainer);
    this.container.appendChild(mainContent);
    this.toastContainer.mount(document.body);

    // Create and mount recency switcher
    this.recencySwitcher = new RecencySwitcher();
    this.recencySwitcher.mount(document.body);

    // Subscribe to state changes
    store.subscribe(() => this.handleStateChange());

    // Setup keyboard shortcuts
    this.setupKeyboardShortcuts();

    // Setup split drop zones for tab drag-drop (pointer-event based)
    this.setupSplitDropZone();
  }

  private handleStateChange() {
    const state = store.getState();

    // Create panes for new terminals
    state.terminals.forEach((terminal) => {
      if (!this.terminalPanes.has(terminal.id)) {
        let pane: TerminalPane | FigmaPane;

        if (terminal.paneType === 'figma' && terminal.figmaUrl) {
          pane = new FigmaPane(terminal.id, terminal.figmaUrl);
        } else {
          pane = new TerminalPane(terminal.id);
        }

        pane.mount(this.terminalContainer);
        this.terminalPanes.set(terminal.id, pane);

        // Load scrollback for restored terminals (only terminal panes)
        if (pane instanceof TerminalPane && this.restoredTerminalIds.has(terminal.id)) {
          // Small delay to ensure terminal is mounted
          setTimeout(() => (pane as TerminalPane).loadScrollback(), 100);
          this.restoredTerminalIds.delete(terminal.id);
        }
      }
    });

    // Remove panes for deleted terminals
    const existingTerminalIds = new Set(state.terminals.map((t) => t.id));
    this.terminalPanes.forEach((pane, id) => {
      if (!existingTerminalIds.has(id)) {
        pane.destroy();
        this.terminalPanes.delete(id);
      }
    });

    // Update active state (split-aware)
    const layoutTree = state.activeWorkspaceId
      ? store.getLayoutTree(state.activeWorkspaceId)
      : null;
    const legacySplit = state.activeWorkspaceId
      ? store.getSplitView(state.activeWorkspaceId)
      : null;

    if (layoutTree) {
      // Recursive split mode: use SplitContainer
      this.terminalContainer.classList.remove('split-horizontal', 'split-vertical');
      this.removeSplitDivider();

      // Hide all panes that are NOT in the layout tree
      const visibleIds = new Set(terminalIds(layoutTree));
      this.terminalPanes.forEach((pane, id) => {
        if (!visibleIds.has(id)) {
          pane.setActive(false);
          pane.getContainer().style.flexBasis = '';
        }
      });

      if (this.splitContainer && this.splitContainerWorkspaceId === state.activeWorkspaceId) {
        // Update existing SplitContainer
        this.splitContainer.update(layoutTree, state.activeTerminalId);
      } else {
        // Create new SplitContainer
        this.destroySplitContainer();
        this.splitContainerWorkspaceId = state.activeWorkspaceId;
        this.splitContainer = new SplitContainer(layoutTree, {
          paneMap: this.terminalPanes as Map<string, import('./SplitContainer').SplitPaneHandle>,
          onRatioChange: (path, ratio) => {
            if (state.activeWorkspaceId) {
              store.updateLayoutTreeRatio(state.activeWorkspaceId, path, ratio);
            }
          },
          onGridRatioChange: (path, key, ratio) => {
            if (state.activeWorkspaceId) {
              store.updateGridRatio(state.activeWorkspaceId, path, key, ratio);
            }
          },
          onFocusPane: (terminalId) => {
            store.setActiveTerminal(terminalId);
          },
          focusedTerminalId: state.activeTerminalId,
        });
        this.terminalContainer.appendChild(this.splitContainer.getElement());
      }
    } else if (legacySplit) {
      // Legacy 2-pane split mode
      this.destroySplitContainer();
      this.terminalContainer.classList.remove('split-horizontal', 'split-vertical');
      this.terminalContainer.classList.add(
        legacySplit.direction === 'horizontal' ? 'split-horizontal' : 'split-vertical'
      );

      const leftPane = this.terminalPanes.get(legacySplit.leftTerminalId);
      const rightPane = this.terminalPanes.get(legacySplit.rightTerminalId);

      // Ensure divider exists
      this.ensureSplitDivider(legacySplit.direction);

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
      const leftBasis = `calc(${legacySplit.ratio * 100}% - 2px)`;
      const rightBasis = `calc(${(1 - legacySplit.ratio) * 100}% - 2px)`;

      this.terminalPanes.forEach((pane, id) => {
        const terminal = state.terminals.find((t) => t.id === id);
        if (terminal?.workspaceId !== state.activeWorkspaceId) {
          pane.setActive(false);
          return;
        }

        if (id === legacySplit.leftTerminalId) {
          pane.getContainer().style.flexBasis = leftBasis;
          pane.setSplitVisible(true, state.activeTerminalId === id);
        } else if (id === legacySplit.rightTerminalId) {
          pane.getContainer().style.flexBasis = rightBasis;
          pane.setSplitVisible(true, state.activeTerminalId === id);
        } else {
          pane.setActive(false);
        }
      });
    } else {
      // Single pane mode
      this.destroySplitContainer();
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

    // Show exited overlay on dead terminals
    for (const terminal of state.terminals) {
      if (terminal.exited) {
        const pane = this.terminalPanes.get(terminal.id);
        if (pane instanceof TerminalPane) {
          pane.showExitedOverlay(terminal.exitCode);
        }
      }
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
    setupKeyboardShortcuts({
      getPerfOverlay: () => this.perfOverlay,
      setPerfOverlay: (overlay) => { this.perfOverlay = overlay; },
      startRenameActive: () => this.tabBar.startRenameActive(),
      createNewTerminal: () => this.createNewTerminal(),
      createSplitTerminal: (dir) => this.createSplitTerminal(dir),
      handleUnsplitRequest: () => this.handleUnsplitRequest(),
      handleVoiceToggle: () => handleVoiceToggle(),
      getZoomedPaneId: () => this.zoomedPaneId,
      setZoomedPaneId: (id) => { this.zoomedPaneId = id; },
      getPreZoomRatio: () => this.preZoomRatio,
      setPreZoomRatio: (ratio) => { this.preZoomRatio = ratio; },
      getRecencySwitcher: () => this.recencySwitcher,
    });
  }

  async init() {
    perfTracer.mark('app_init_start');
    // Initialize terminal service
    await terminalService.init();

    // Listen for quit confirmation requests from backend (on window close)
    await this.setupConfirmQuitListener();

    // Listen for scrollback save requests from backend (on window close)
    await this.setupScrollbackSaveListener();

    // Listen for file drag-drop events (paste full file paths into terminal)
    await this.setupDragDropListener();

    // Listen for MCP-triggered UI events
    await this.setupMcpEventListeners();

    // Load persisted state and reconnect sessions
    await restoreLayout({
      markRestoredTerminal: (id) => this.restoredTerminalIds.add(id),
      markReattachedTerminal: (id) => this.reattachedTerminalIds.add(id),
    });

    perfTracer.measure('app_startup', 'app_init_start');
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
        // Guard: if button was released outside the window, clean up
        if (moveEvent.buttons === 0) { onMouseUp(); return; }

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
          await syncSplitToBackend(state.activeWorkspaceId!, currentSplit);
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

  private destroySplitContainer() {
    if (this.splitContainer) {
      this.splitContainer.destroy();
      this.splitContainer = null;
      this.splitContainerWorkspaceId = null;
    }
  }

  private setupSplitDropZone() {
    // Create drop overlay element (reused from previous implementation)
    this.splitDropOverlay = document.createElement('div');
    this.splitDropOverlay.className = 'split-drop-overlay';
    this.terminalContainer.appendChild(this.splitDropOverlay);

    // Pointer-event drag move: show split zone overlay for tab drags
    onDragMove((x, y, data) => {
      if (data.kind !== 'tab') return;

      const rect = this.terminalContainer.getBoundingClientRect();
      // Ignore if pointer is outside the terminal container
      if (x < rect.left || x > rect.right || y < rect.top || y > rect.bottom) {
        this.splitDropOverlay?.classList.remove('visible');
        if (this.splitDropOverlay) this.splitDropOverlay.dataset.zone = '';
        return;
      }

      const nx = (x - rect.left) / rect.width;
      const ny = (y - rect.top) / rect.height;
      const overlay = this.splitDropOverlay!;

      if (nx > 0.7) {
        overlay.style.left = '50%';
        overlay.style.top = '0';
        overlay.style.width = '50%';
        overlay.style.height = '100%';
        overlay.textContent = 'Split Right';
        overlay.classList.add('visible');
        overlay.dataset.zone = 'right';
      } else if (nx < 0.3) {
        overlay.style.left = '0';
        overlay.style.top = '0';
        overlay.style.width = '50%';
        overlay.style.height = '100%';
        overlay.textContent = 'Split Left';
        overlay.classList.add('visible');
        overlay.dataset.zone = 'left';
      } else if (ny > 0.7) {
        overlay.style.left = '0';
        overlay.style.top = '50%';
        overlay.style.width = '100%';
        overlay.style.height = '50%';
        overlay.textContent = 'Split Down';
        overlay.classList.add('visible');
        overlay.dataset.zone = 'bottom';
      } else if (ny < 0.3) {
        overlay.style.left = '0';
        overlay.style.top = '0';
        overlay.style.width = '100%';
        overlay.style.height = '50%';
        overlay.textContent = 'Split Up';
        overlay.classList.add('visible');
        overlay.dataset.zone = 'top';
      } else {
        overlay.classList.remove('visible');
        overlay.dataset.zone = '';
      }
    });

    // Pointer-event drag drop: handle split creation
    onDragDrop((_x, _y, data) => {
      if (data.kind !== 'tab') return;

      this.splitDropOverlay?.classList.remove('visible');
      const zone = this.splitDropOverlay?.dataset.zone;
      if (!zone) return;

      const droppedTerminalId = data.id;
      const state = store.getState();
      if (!state.activeWorkspaceId || !state.activeTerminalId) return;

      // Don't split with the same terminal
      if (droppedTerminalId === state.activeTerminalId) return;

      // Verify the dropped terminal belongs to this workspace
      const droppedTerminal = state.terminals.find(t => t.id === droppedTerminalId);
      if (!droppedTerminal || droppedTerminal.workspaceId !== state.activeWorkspaceId) return;

      // Determine split direction from drop zone
      const direction: 'horizontal' | 'vertical' =
        (zone === 'left' || zone === 'right') ? 'horizontal' : 'vertical';

      // Use layout tree model: split the focused pane with the dropped terminal
      const targetId = state.activeTerminalId;

      // For right/bottom drops, the new terminal goes in the "second" slot (default).
      // For left/top drops, we need to swap the order after splitting.
      store.splitTerminalAt(state.activeWorkspaceId, targetId, droppedTerminalId, direction);

      // For left/top drops, swap the terminals so the dropped one appears first
      if (zone === 'left' || zone === 'top') {
        const tree = store.getLayoutTree(state.activeWorkspaceId);
        if (tree) {
          const newTree = swapTerminals(tree, targetId, droppedTerminalId);
          if (newTree) {
            store.setLayoutTree(state.activeWorkspaceId, newTree);
          }
        }
      }
    });
  }


  /**
   * Create a new terminal in the active workspace. Returns the new terminal ID,
   * or null if creation was cancelled (e.g. worktree prompt dismissed).
   */
  private async createNewTerminal(): Promise<string | null> {
    const state = store.getState();
    if (!state.activeWorkspaceId) return null;

    perfTracer.mark('create_terminal_start');
    const workspace = state.workspaces.find(w => w.id === state.activeWorkspaceId);
    let worktreeName: string | undefined;

    if (workspace?.worktreeMode) {
      const { showWorktreeNamePrompt } = await import('./dialogs');
      const name = await showWorktreeNamePrompt();
      if (name === null) return null;
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
      processName: shellTypeToProcessName(terminalSettingsStore.getDefaultShell()),
      order: 0,
    });
    perfTracer.measure('create_terminal', 'create_terminal_start');

    const aiMode = workspace?.aiToolMode;
    if (aiMode === 'both') {
      // Both mode: delegate to dedicated method that creates 2 terminals + split
      return this.createNewTerminalBothMode(workspace!);
    }

    if (aiMode === 'claude') {
      setTimeout(() => {
        terminalService.writeToTerminal(result.id, 'claude --dangerously-skip-permissions\r');
      }, 500);
    } else if (aiMode === 'codex') {
      setTimeout(() => {
        terminalService.writeToTerminal(result.id, 'codex --yolo\r');
      }, 500);
    }

    return result.id;
  }

  /**
   * Both mode: create two terminals (Claude + Codex) in a vertical split.
   * Mirrors TabBar.handleNewTabBothMode().
   */
  private async createNewTerminalBothMode(workspace: import('../state/store').Workspace): Promise<string> {
    const wsId = workspace.id;
    let worktreeNameClaude: string | undefined;
    let worktreeNameCodex: string | undefined;

    if (workspace.worktreeMode) {
      const { showWorktreeNamePrompt } = await import('./dialogs');
      const baseName = await showWorktreeNamePrompt('Enter worktree base name (suffixes -claude/-codex added)');
      if (baseName === null) return wsId; // user cancelled — return workspace ID as fallback
      if (baseName) {
        worktreeNameClaude = `${baseName}-claude`;
        worktreeNameCodex = `${baseName}-codex`;
      }
    }

    // Create first terminal (Claude)
    const result1 = await terminalService.createTerminal(wsId, { worktreeName: worktreeNameClaude });
    store.addTerminal({
      id: result1.id,
      workspaceId: wsId,
      name: result1.worktree_branch ?? 'Claude',
      processName: shellTypeToProcessName(terminalSettingsStore.getDefaultShell()),
      order: 0,
    });

    // Create second terminal (Codex)
    const result2 = await terminalService.createTerminal(wsId, { worktreeName: worktreeNameCodex });
    store.addTerminal({
      id: result2.id,
      workspaceId: wsId,
      name: result2.worktree_branch ?? 'Codex',
      processName: shellTypeToProcessName(terminalSettingsStore.getDefaultShell()),
      order: 0,
    }, { background: true });

    // Split vertically
    store.splitTerminalAt(wsId, result1.id, result2.id, 'vertical', 0.5);

    // Write commands after delay
    setTimeout(() => {
      terminalService.writeToTerminal(result1.id, 'claude --dangerously-skip-permissions\r');
    }, 500);
    setTimeout(() => {
      terminalService.writeToTerminal(result2.id, 'codex --yolo\r');
    }, 500);

    return result1.id;
  }

  private async createSplitTerminal(direction: 'horizontal' | 'vertical') {
    const state = store.getState();
    if (!state.activeWorkspaceId || !state.activeTerminalId) return;

    const workspace = state.workspaces.find(w => w.id === state.activeWorkspaceId);
    const currentActiveId = state.activeTerminalId;
    const newId = await this.createNewTerminal();
    if (!newId) return;

    // Both mode already creates a split in createNewTerminalBothMode()
    if (workspace?.aiToolMode === 'both') return;

    // Use layout tree model
    store.splitTerminalAt(state.activeWorkspaceId, currentActiveId, newId, direction);
  }

  private async handleSplitRequest(terminalId: string, direction: 'horizontal' | 'vertical') {
    const state = store.getState();
    if (!state.activeWorkspaceId || !state.activeTerminalId) return;
    if (terminalId === state.activeTerminalId) return;

    // Use layout tree model: create a 2-pane split with active + dropped terminal
    store.splitTerminalAt(state.activeWorkspaceId, state.activeTerminalId, terminalId, direction);
  }

  private async handleUnsplitRequest() {
    const state = store.getState();
    if (!state.activeWorkspaceId) return;

    // Clear layout tree first, then legacy split
    const tree = store.getLayoutTree(state.activeWorkspaceId);
    if (tree) {
      store.unsplitTerminal(state.activeWorkspaceId);
    }
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

  private async setupConfirmQuitListener() {
    const { listen } = await import('@tauri-apps/api/event');
    await listen<{ active_session_count: number }>('confirm-quit', async (event) => {
      const { active_session_count } = event.payload;
      const { invoke } = await import('@tauri-apps/api/core');

      // Skip dialog if setting is disabled or no active sessions
      if (!terminalSettingsStore.getConfirmQuit() || active_session_count === 0) {
        await invoke('confirm_quit');
        return;
      }

      // Show confirmation dialog
      const { showQuitConfirmDialog } = await import('./dialogs');
      const confirmed = await showQuitConfirmDialog(active_session_count);

      if (confirmed) {
        await invoke('confirm_quit');
      } else {
        await invoke('cancel_quit');
      }
    });
  }

  private async setupScrollbackSaveListener() {
    const { listen } = await import('@tauri-apps/api/event');
    await listen('request-scrollback-save', async () => {
      console.log('[App] Saving all scrollbacks before exit...');
      const saves = Array.from(this.terminalPanes.values())
        .filter((pane): pane is TerminalPane => pane instanceof TerminalPane)
        .map((pane) => pane.saveScrollback());
      await Promise.all(saves);
      console.log('[App] All scrollbacks saved, signaling completion...');

      // Signal completion to backend
      const { invoke } = await import('@tauri-apps/api/core');
      await invoke('scrollback_save_complete');
    });
  }

  private async setupDragDropListener() {
    // File drop handling via Tauri's native drag-drop events.
    // With dragDropEnabled: true, Tauri's IDropTarget intercepts external file
    // drags and provides full file system paths. Internal DnD (tab reorder,
    // workspace reorder, split zones) uses pointer events instead of HTML5 DnD
    // to avoid conflict with Tauri's IDropTarget (see drag-state.ts).
    const { getCurrentWebviewWindow } = await import('@tauri-apps/api/webviewWindow');

    await getCurrentWebviewWindow().onDragDropEvent((event) => {
      // Skip when a dialog overlay is open (e.g., Quick Claude handles its own drops)
      if (document.querySelector('.dialog-overlay')) return;

      if (event.payload.type === 'enter') {
        this.terminalContainer.classList.add('drag-file-over');
      } else if (event.payload.type === 'leave') {
        this.terminalContainer.classList.remove('drag-file-over');
      } else if (event.payload.type === 'drop') {
        this.terminalContainer.classList.remove('drag-file-over');

        const state = store.getState();
        if (!state.activeTerminalId || !event.payload.paths.length) return;

        // Don't paste file paths into Figma panes
        const activeTerminal = state.terminals.find(t => t.id === state.activeTerminalId);
        if (activeTerminal?.paneType === 'figma') return;

        const text = event.payload.paths.map(quotePath).join(' ');
        terminalService.writeToTerminal(state.activeTerminalId, text);
      }
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
      store.updateTerminal(terminal_id, { name, userRenamed: true });
    });

    // MCP: terminal created by MCP handler — add to main window's Agent workspace
    await listen<{ terminal_id: string; workspace_id: string }>('mcp-terminal-created', async (event) => {
      const { terminal_id: terminalId, workspace_id: workspaceId } = event.payload;
      console.log('[App] MCP terminal created:', terminalId, 'in workspace:', workspaceId);

      const state = store.getState();
      const existing = state.terminals.find((t) => t.id === terminalId);
      if (!existing) {
        // Ensure Agent workspace exists in store
        if (!state.workspaces.find(w => w.id === workspaceId)) {
          store.addWorkspace({
            id: workspaceId,
            name: 'Agent',
            folderPath: '',
            tabOrder: [],
            shellType: { type: 'windows' },
            worktreeMode: false,
            aiToolMode: 'none',
          });
        }
        store.addTerminal({
          id: terminalId,
          workspaceId,
          name: 'Terminal',
          processName: shellTypeToProcessName(terminalSettingsStore.getDefaultShell()),
          order: 0,
        });
      }
    });

    // Quick Claude: session ready, prompt delivered
    await listen<{ terminal_id: string; display_name: string }>('quick-claude-ready', (event) => {
      const { terminal_id, display_name } = event.payload;
      this.toastContainer.show('Claude Ready', `${display_name} — prompt delivered`, terminal_id);

      // Emit plugin event
      const registry = getPluginRegistry();
      if (registry) {
        registry.getBus().emit({
          type: 'agent:ready',
          terminalId: terminal_id,
          message: `${display_name} — prompt delivered`,
          timestamp: Date.now(),
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
      if (isWorkspaceNotificationSuppressed(terminal_id)) return;

      const state = store.getState();
      const isActive = state.activeTerminalId === terminal_id;

      const played = notificationStore.recordNotify(terminal_id);
      if (played) {
        // Emit plugin event; if a plugin handled the sound, skip the default
        const registry = getPluginRegistry();
        let pluginHandledSound = false;
        if (registry) {
          const result = registry.getBus().emitMcpNotify(terminal_id, message);
          pluginHandledSound = result.soundHandled;
        }
        if (!pluginHandledSound) {
          playNotificationSound(settings.soundPreset, settings.volume);
        }

        // Show in-app toast (always, regardless of focus)
        const toastTitle = buildNotificationTitle(terminal_id);
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
              const title = buildNotificationTitle(terminal_id);
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

    // MCP: split view created by MCP handler
    await listen<{
      workspace_id: string;
      left_terminal_id: string;
      right_terminal_id: string;
      direction: string;
      ratio: number;
    }>('mcp-set-split-view', (event) => {
      const { workspace_id, left_terminal_id, right_terminal_id, direction, ratio } = event.payload;
      console.log('[App] MCP set split view:', workspace_id, direction);
      const dir = direction === 'vertical' ? 'vertical' : 'horizontal';
      // Create layout tree from MCP split view
      const tree = fromLegacySplitView({
        leftTerminalId: left_terminal_id,
        rightTerminalId: right_terminal_id,
        direction: dir,
        ratio,
      });
      store.setLayoutTree(workspace_id, tree);
      // Also set legacy split for backend persistence
      store.setSplitView(workspace_id, left_terminal_id, right_terminal_id, dir, ratio);
    });

    // MCP: split view cleared by MCP handler
    await listen<string>('mcp-clear-split-view', (event) => {
      const workspaceId = event.payload;
      console.log('[App] MCP clear split view:', workspaceId);
      store.clearLayoutTree(workspaceId);
      store.clearSplitView(workspaceId);
    });

    // MCP: split terminal (layout tree split)
    await listen<{
      workspace_id: string;
      target_terminal_id: string;
      new_terminal_id: string;
      direction: string;
      ratio: number;
    }>('mcp-split-terminal', (event) => {
      const { workspace_id, target_terminal_id, new_terminal_id, direction, ratio } = event.payload;
      console.log('[App] MCP split terminal:', workspace_id, target_terminal_id, direction);
      const dir = direction === 'vertical' ? 'vertical' : 'horizontal';
      store.splitTerminalAt(workspace_id, target_terminal_id, new_terminal_id, dir, ratio);
    });

    // MCP: unsplit terminal
    await listen<{
      workspace_id: string;
      terminal_id: string;
    }>('mcp-unsplit-terminal', (event) => {
      const { workspace_id, terminal_id } = event.payload;
      console.log('[App] MCP unsplit terminal:', workspace_id, terminal_id);
      store.unsplitTerminal(workspace_id, terminal_id);
    });

    // MCP: swap panes
    await listen<{
      workspace_id: string;
      terminal_id_a: string;
      terminal_id_b: string;
    }>('mcp-swap-panes', (event) => {
      const { workspace_id, terminal_id_a, terminal_id_b } = event.payload;
      console.log('[App] MCP swap panes:', workspace_id, terminal_id_a, terminal_id_b);
      store.swapPanes(workspace_id, terminal_id_a, terminal_id_b);
    });

    // MCP: zoom pane
    await listen<{
      workspace_id: string;
      terminal_id: string | null;
    }>('mcp-zoom-pane', (event) => {
      const { workspace_id, terminal_id } = event.payload;
      console.log('[App] MCP zoom pane:', workspace_id, terminal_id);
      store.setZoomedPane(workspace_id, terminal_id);
    });

    // Terminal bell (BEL character, 0x07) — triggers notification pipeline
    await listen<{ terminal_id: string }>('terminal-bell', async (event) => {
      const { terminal_id } = event.payload;
      const settings = notificationStore.getSettings();
      if (!settings.globalEnabled) return;
      if (isWorkspaceNotificationSuppressed(terminal_id)) return;

      const state = store.getState();
      const isActive = state.activeTerminalId === terminal_id;

      const played = notificationStore.recordNotify(terminal_id);
      if (played) {
        playNotificationSound(settings.soundPreset, settings.volume);

        const toastTitle = buildNotificationTitle(terminal_id);
        this.toastContainer.show(toastTitle, 'Terminal bell', terminal_id);

        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        getCurrentWindow().requestUserAttention(2);

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
              const title = buildNotificationTitle(terminal_id);
              sendNotification({ title, body: 'Terminal bell' });
            }
          } catch (e) {
            console.warn('[App] Failed to send native notification:', e);
          }
        }
      }

      if (isActive) {
        notificationStore.clearBadge(terminal_id);
      }
    });

    // Idle-after-activity detection: monitors terminals for output→silence transitions
    const idleService = new IdleNotificationService({
      idleThresholdMs: 30000,
      checkIntervalMs: 5000,
      startupGraceMs: 20000,
      notifyCooldownMs: 60000,
      minOutputEvents: 10,
      getActiveTerminalId: () => store.getState().activeTerminalId ?? undefined,
      onNotify: async (terminalId: string) => {
        const settings = notificationStore.getSettings();
        if (!settings.globalEnabled || !settings.idleNotifyEnabled) return;
        if (isWorkspaceNotificationSuppressed(terminalId)) return;

        const played = notificationStore.recordNotify(terminalId);
        if (played) {
          playNotificationSound(settings.soundPreset, settings.volume);

          const toastTitle = buildNotificationTitle(terminalId);
          this.toastContainer.show(toastTitle, 'Waiting for input', terminalId);

          const { getCurrentWindow } = await import('@tauri-apps/api/window');
          getCurrentWindow().requestUserAttention(2);

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
                const title = buildNotificationTitle(terminalId);
                sendNotification({ title, body: 'Waiting for input' });
              }
            } catch (e) {
              console.warn('[App] Failed to send native notification:', e);
            }
          }
        }
      },
    });

    // Wire terminal output/grid-diff events to idle tracker
    await listen<{ terminal_id: string }>('terminal-output', (event) => {
      idleService.recordOutput(event.payload.terminal_id);
    });
    await listen<{ terminal_id: string }>('terminal-grid-diff', (event) => {
      idleService.recordOutput(event.payload.terminal_id);
    });
    await listen<{ terminal_id: string }>('terminal-closed', (event) => {
      idleService.recordTerminalClosed(event.payload.terminal_id);
    });
  }
}
