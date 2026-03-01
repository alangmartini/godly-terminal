import { store } from '../state/store';
import { terminalSettingsStore } from '../state/terminal-settings-store';
import { terminalService } from '../services/terminal-service';
import { workspaceService } from '../services/workspace-service';
import { keybindingStore } from '../state/keybinding-store';
import { perfTracer } from '../utils/PerfTracer';
import { PerfOverlay } from '../components/PerfOverlay';
import { shellTypeToProcessName } from '../utils/shell-type-utils';
import { terminalIds } from '../state/split-types';

export interface KeyboardDeps {
  /** Returns current perfOverlay instance (may be null). */
  getPerfOverlay(): PerfOverlay | null;
  /** Sets the perfOverlay instance. */
  setPerfOverlay(overlay: PerfOverlay | null): void;
  /** Triggers tab rename on the active tab. */
  startRenameActive(): void;
  /** Creates a new terminal, returns its ID or null. */
  createNewTerminal(): Promise<string | null>;
  /** Creates a split terminal in the given direction. */
  createSplitTerminal(direction: 'horizontal' | 'vertical'): Promise<void>;
  /** Handles unsplit request. */
  handleUnsplitRequest(): void;
  /** Handles voice recording toggle. */
  handleVoiceToggle(): Promise<void>;
  /** Tracks zoomed pane state. */
  getZoomedPaneId(): string | null;
  setZoomedPaneId(id: string | null): void;
  getPreZoomRatio(): number | null;
  setPreZoomRatio(ratio: number | null): void;
}

export function setupKeyboardShortcuts(deps: KeyboardDeps): void {
  document.addEventListener('keydown', async (e) => {
    const state = store.getState();

    // -- Hardcoded shortcuts (not customisable) --

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
      const { showSettingsDialog } = await import('../components/SettingsDialog');
      await showSettingsDialog();
      return;
    }

    // -- Dynamic shortcuts (customisable via settings) --

    const action = keybindingStore.matchAction(e);
    if (!action) return;

    switch (action) {
      case 'debug.togglePerfOverlay': {
        e.preventDefault();
        const overlay = deps.getPerfOverlay();
        if (overlay) {
          overlay.destroy();
          deps.setPerfOverlay(null);
        } else {
          const newOverlay = new PerfOverlay();
          newOverlay.mount(document.body);
          deps.setPerfOverlay(newOverlay);
        }
        break;
      }

      case 'tabs.newTerminal': {
        e.preventDefault();
        await deps.createNewTerminal();
        break;
      }

      case 'tabs.closeTerminal': {
        e.preventDefault();
        if (state.activeTerminalId) {
          const terminal = state.terminals.find(t => t.id === state.activeTerminalId);
          if (terminal?.paneType !== 'figma') {
            await terminalService.closeTerminal(state.activeTerminalId);
          }
          store.removeTerminal(state.activeTerminalId);
        }
        break;
      }

      case 'tabs.nextTab': {
        e.preventDefault();
        perfTracer.mark('tab_switch_start');
        const terminals = store.getWorkspaceTerminals(
          state.activeWorkspaceId || ''
        );
        if (terminals.length > 1 && state.activeTerminalId) {
          const currentIndex = terminals.findIndex(
            (t) => t.id === state.activeTerminalId
          );
          const nextIndex = (currentIndex + 1) % terminals.length;
          store.setActiveTerminal(terminals[nextIndex].id);
          perfTracer.measure('tab_switch', 'tab_switch_start');
        }
        break;
      }

      case 'tabs.previousTab': {
        e.preventDefault();
        perfTracer.mark('tab_switch_start');
        const terminals = store.getWorkspaceTerminals(
          state.activeWorkspaceId || ''
        );
        if (terminals.length > 1 && state.activeTerminalId) {
          const currentIndex = terminals.findIndex(
            (t) => t.id === state.activeTerminalId
          );
          const nextIndex = (currentIndex - 1 + terminals.length) % terminals.length;
          store.setActiveTerminal(terminals[nextIndex].id);
          perfTracer.measure('tab_switch', 'tab_switch_start');
        }
        break;
      }

      case 'tabs.renameTerminal': {
        e.preventDefault();
        deps.startRenameActive();
        break;
      }

      case 'split.focusOtherPane': {
        e.preventDefault();
        if (state.activeWorkspaceId && state.activeTerminalId) {
          // Layout tree: cycle through panes in tree order
          const tree = store.getLayoutTree(state.activeWorkspaceId);
          if (tree) {
            const ids = terminalIds(tree);
            const currentIdx = ids.indexOf(state.activeTerminalId);
            if (currentIdx >= 0 && ids.length > 1) {
              const nextIdx = (currentIdx + 1) % ids.length;
              store.setActiveTerminal(ids[nextIdx]);
            }
          } else {
            // Legacy split
            const activeSplit = store.getSplitView(state.activeWorkspaceId);
            if (activeSplit) {
              const otherId = state.activeTerminalId === activeSplit.leftTerminalId
                ? activeSplit.rightTerminalId
                : activeSplit.leftTerminalId;
              store.setActiveTerminal(otherId);
            }
          }
        }
        break;
      }

      case 'split.splitRight': {
        e.preventDefault();
        await deps.createSplitTerminal('horizontal');
        break;
      }

      case 'split.splitDown': {
        e.preventDefault();
        await deps.createSplitTerminal('vertical');
        break;
      }

      case 'split.unsplit': {
        e.preventDefault();
        deps.handleUnsplitRequest();
        break;
      }

      case 'split.focusLeft':
      case 'split.focusRight':
      case 'split.focusUp':
      case 'split.focusDown': {
        e.preventDefault();
        if (state.activeWorkspaceId && state.activeTerminalId) {
          const direction = (action === 'split.focusLeft' || action === 'split.focusRight')
            ? 'horizontal' : 'vertical';
          const goSecond = action === 'split.focusRight' || action === 'split.focusDown';

          // Try layout tree first (supports arbitrary nesting)
          const adjacent = store.getAdjacentPane(
            state.activeWorkspaceId, state.activeTerminalId, direction, goSecond,
          );
          if (adjacent) {
            store.setActiveTerminal(adjacent);
          } else {
            // Fallback to legacy 2-pane split
            const split = store.getSplitView(state.activeWorkspaceId);
            if (split) {
              const isMatch = (direction === 'horizontal' && split.direction === 'horizontal')
                || (direction === 'vertical' && split.direction === 'vertical');
              const targetId = isMatch
                ? (goSecond ? split.rightTerminalId : split.leftTerminalId)
                : null;
              if (targetId && targetId !== state.activeTerminalId) {
                store.setActiveTerminal(targetId);
              }
            }
          }
        }
        break;
      }

      case 'split.resizeLeft':
      case 'split.resizeRight':
      case 'split.resizeUp':
      case 'split.resizeDown': {
        e.preventDefault();
        if (state.activeWorkspaceId) {
          const split = store.getSplitView(state.activeWorkspaceId);
          if (split) {
            const isHorizontal = split.direction === 'horizontal';
            const isVertical = split.direction === 'vertical';
            const RESIZE_STEP = 0.05;
            let delta = 0;

            if (action === 'split.resizeLeft' && isHorizontal) delta = -RESIZE_STEP;
            else if (action === 'split.resizeRight' && isHorizontal) delta = RESIZE_STEP;
            else if (action === 'split.resizeUp' && isVertical) delta = -RESIZE_STEP;
            else if (action === 'split.resizeDown' && isVertical) delta = RESIZE_STEP;

            if (delta !== 0) {
              const newRatio = Math.max(0.1, Math.min(0.9, split.ratio + delta));
              store.updateSplitRatio(state.activeWorkspaceId, newRatio);
            }
          }
        }
        break;
      }

      case 'split.zoom': {
        e.preventDefault();
        if (state.activeWorkspaceId && state.activeTerminalId) {
          const split = store.getSplitView(state.activeWorkspaceId);
          if (split) {
            if (deps.getZoomedPaneId()) {
              // Unzoom: restore split ratio
              store.updateSplitRatio(state.activeWorkspaceId, deps.getPreZoomRatio() ?? 0.5);
              deps.setZoomedPaneId(null);
              deps.setPreZoomRatio(null);
            } else {
              // Zoom: save ratio, then push active pane to near-full width
              deps.setPreZoomRatio(split.ratio);
              deps.setZoomedPaneId(state.activeTerminalId);
              const isLeft = state.activeTerminalId === split.leftTerminalId;
              store.updateSplitRatio(state.activeWorkspaceId, isLeft ? 0.95 : 0.05);
            }
          }
        }
        break;
      }

      case 'split.swapPanes': {
        e.preventDefault();
        if (state.activeWorkspaceId) {
          const split = store.getSplitView(state.activeWorkspaceId);
          if (split) {
            store.setSplitView(
              state.activeWorkspaceId,
              split.rightTerminalId,
              split.leftTerminalId,
              split.direction,
              1 - split.ratio,
            );
          }
        }
        break;
      }

      case 'split.rotateSplit': {
        e.preventDefault();
        if (state.activeWorkspaceId) {
          const split = store.getSplitView(state.activeWorkspaceId);
          if (split) {
            const newDirection = split.direction === 'horizontal' ? 'vertical' : 'horizontal';
            store.setSplitView(
              state.activeWorkspaceId,
              split.leftTerminalId,
              split.rightTerminalId,
              newDirection,
              split.ratio,
            );
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

      case 'zoom.in': {
        e.preventDefault();
        const current = terminalSettingsStore.getFontSize();
        terminalSettingsStore.setFontSize(current + 1);
        break;
      }

      case 'zoom.out': {
        e.preventDefault();
        const current = terminalSettingsStore.getFontSize();
        terminalSettingsStore.setFontSize(current - 1);
        break;
      }

      case 'zoom.reset': {
        e.preventDefault();
        terminalSettingsStore.setFontSize(13);
        break;
      }

      case 'voice.toggleRecording': {
        e.preventDefault();
        deps.handleVoiceToggle();
        break;
      }

      case 'tabs.quickClaude': {
        e.preventDefault();
        if (!state.activeWorkspaceId) break;

        const { showQuickClaudeDialog } = await import('../components/dialogs');
        const input = await showQuickClaudeDialog({
          workspaces: state.workspaces.map(w => ({ id: w.id, name: w.name, folderPath: w.folderPath })),
          activeWorkspaceId: state.activeWorkspaceId,
        });
        if (!input) break;

        try {
          const { invoke } = await import('@tauri-apps/api/core');
          const result = await invoke<{ terminal_id: string; worktree_branch: string | null }>(
            'quick_claude',
            {
              workspaceId: input.workspaceId,
              prompt: input.prompt,
              branchName: input.branchName ?? null,
              skipFetch: true,
              noWorktree: input.noWorktree ?? false,
            }
          );

          store.addTerminal({
            id: result.terminal_id,
            workspaceId: input.workspaceId,
            name: result.worktree_branch ?? 'Quick Claude',
            processName: shellTypeToProcessName(terminalSettingsStore.getDefaultShell()),
            order: 0,
          }, { background: true });
        } catch (error) {
          console.error('[App] Quick Claude failed:', error);
        }
        break;
      }
    }
  });

  // Listen for voice toggle events from the mic button in TabBar
  document.addEventListener('voice-toggle-recording', () => deps.handleVoiceToggle());
}
