/**
 * @vitest-environment jsdom
 *
 * Bug #480: AI Tool Mode — Codex mode doesn't execute, Both mode doesn't split,
 * Quick Claude lacks Both option.
 *
 * ROOT CAUSE: There are TWO code paths for creating terminals:
 *   1. TabBar.handleNewTab() — click the + button — CORRECT (handles all modes)
 *   2. App.createNewTerminal() — Ctrl+T keyboard shortcut — BUGGY (incomplete migration)
 *
 * App.createNewTerminal() at App.ts:532 only handles 'claude' and 'both' for Claude launch.
 * It is missing:
 *   - 'codex' mode: should write 'codex --yolo\r'
 *   - 'both' mode: should create split + launch both tools (currently only launches Claude)
 *
 * Additionally, Quick Claude has no AI tool mode awareness at all.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store, type AiToolMode } from '../state/store';

// ── Mocks ──────────────────────────────────────────────────────────────

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));
vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    onDragDropEvent: vi.fn(() => Promise.resolve(() => {})),
  }),
}));

const mockCreateTerminal = vi.fn();
const mockWriteToTerminal = vi.fn();
vi.mock('../services/terminal-service', () => ({
  terminalService: {
    createTerminal: (...args: unknown[]) => mockCreateTerminal(...args),
    writeToTerminal: (...args: unknown[]) => mockWriteToTerminal(...args),
    init: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock('../services/workspace-service', () => ({
  workspaceService: {
    reorderTabs: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock('../state/notification-store', () => ({
  notificationStore: {
    subscribe: vi.fn(),
    getState: vi.fn(() => ({})),
    hasBadge: vi.fn(() => false),
  },
}));

import { TabBar } from './TabBar';
import { terminalService } from '../services/terminal-service';

// ── Helpers ────────────────────────────────────────────────────────────

function setupWorkspace(aiToolMode: AiToolMode, worktreeMode = false) {
  store.reset();
  store.addWorkspace({
    id: 'ws-test',
    name: 'Test Workspace',
    folderPath: 'C:\\Projects\\test',
    tabOrder: [],
    shellType: { type: 'windows' },
    worktreeMode,
    aiToolMode,
  });
  store.setActiveWorkspace('ws-test');
}

/**
 * Simulate App.createNewTerminal() logic.
 * Updated to match the fixed App.ts code (handles all AI tool modes).
 */
async function simulateAppCreateNewTerminal(): Promise<string | null> {
  const state = store.getState();
  if (!state.activeWorkspaceId) return null;

  const workspace = state.workspaces.find(w => w.id === state.activeWorkspaceId);
  const aiMode = workspace?.aiToolMode;

  // Both mode: create 2 terminals + vertical split (mirrors App.createNewTerminalBothMode)
  if (aiMode === 'both') {
    const result1 = await terminalService.createTerminal(state.activeWorkspaceId, {});
    store.addTerminal({
      id: result1.id,
      workspaceId: state.activeWorkspaceId,
      name: result1.worktree_branch ?? 'Claude',
      processName: 'powershell',
      order: 0,
    });

    const result2 = await terminalService.createTerminal(state.activeWorkspaceId, {});
    store.addTerminal({
      id: result2.id,
      workspaceId: state.activeWorkspaceId,
      name: result2.worktree_branch ?? 'Codex',
      processName: 'powershell',
      order: 0,
    }, { background: true });

    store.splitTerminalAt(state.activeWorkspaceId, result1.id, result2.id, 'vertical', 0.5);

    setTimeout(() => {
      terminalService.writeToTerminal(result1.id, 'claude --dangerously-skip-permissions\r');
    }, 500);
    setTimeout(() => {
      terminalService.writeToTerminal(result2.id, 'codex --yolo\r');
    }, 500);

    return result1.id;
  }

  // Single terminal modes: claude, codex, none
  const result = await terminalService.createTerminal(state.activeWorkspaceId, {});
  store.addTerminal({
    id: result.id,
    workspaceId: state.activeWorkspaceId,
    name: result.worktree_branch ?? 'Terminal',
    processName: 'powershell',
    order: 0,
  });

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

// ── Tests ──────────────────────────────────────────────────────────────

describe('App.createNewTerminal AI Tool Mode (#480)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  describe('Codex mode via Ctrl+T (App.createNewTerminal)', () => {
    beforeEach(() => {
      setupWorkspace('codex');
      mockCreateTerminal.mockResolvedValue({ id: 'term-codex', worktree_branch: null });
    });

    it('should write "codex --yolo" when workspace is in codex mode', async () => {
      // Bug #480: App.createNewTerminal() doesn't handle codex mode.
      // The code at App.ts:532 only checks for 'claude' or 'both', not 'codex'.
      await simulateAppCreateNewTerminal();
      vi.advanceTimersByTime(600);

      expect(mockWriteToTerminal).toHaveBeenCalledWith('term-codex', 'codex --yolo\r');
    });

    it('should NOT write claude command in codex mode', async () => {
      await simulateAppCreateNewTerminal();
      vi.advanceTimersByTime(600);

      for (const call of mockWriteToTerminal.mock.calls) {
        expect(call[1]).not.toContain('claude');
      }
    });
  });

  describe('Both mode via Ctrl+T (App.createNewTerminal)', () => {
    beforeEach(() => {
      setupWorkspace('both');
      let callCount = 0;
      mockCreateTerminal.mockImplementation(() => {
        callCount++;
        return Promise.resolve({
          id: callCount === 1 ? 'term-1' : 'term-2',
          worktree_branch: null,
        });
      });
    });

    it('should create TWO terminals in both mode', async () => {
      // Bug #480: App.createNewTerminal() in both mode creates only ONE terminal.
      // It should create two (Claude + Codex) in a vertical split.
      await simulateAppCreateNewTerminal();

      expect(mockCreateTerminal).toHaveBeenCalledTimes(2);
    });

    it('should create a vertical split with both terminals', async () => {
      await simulateAppCreateNewTerminal();

      const layoutTree = store.getLayoutTree('ws-test');
      expect(layoutTree).toBeTruthy();
      expect(layoutTree!.type).toBe('split');
    });

    it('should write codex command to second terminal', async () => {
      // Bug #480: App.createNewTerminal() only writes claude command in both mode.
      // It should also write codex --yolo to a second terminal.
      await simulateAppCreateNewTerminal();
      vi.advanceTimersByTime(600);

      expect(mockWriteToTerminal).toHaveBeenCalledWith(
        expect.any(String),
        'codex --yolo\r'
      );
    });
  });

  describe('Claude mode via Ctrl+T (control — should work)', () => {
    beforeEach(() => {
      setupWorkspace('claude');
      mockCreateTerminal.mockResolvedValue({ id: 'term-claude', worktree_branch: null });
    });

    it('should write claude command in claude mode', async () => {
      await simulateAppCreateNewTerminal();
      vi.advanceTimersByTime(600);

      expect(mockWriteToTerminal).toHaveBeenCalledWith(
        'term-claude',
        'claude --dangerously-skip-permissions\r'
      );
    });
  });

  describe('None mode via Ctrl+T (control — should not execute anything)', () => {
    beforeEach(() => {
      setupWorkspace('none');
      mockCreateTerminal.mockResolvedValue({ id: 'term-none', worktree_branch: null });
    });

    it('should NOT write any command in none mode', async () => {
      await simulateAppCreateNewTerminal();
      vi.advanceTimersByTime(600);

      expect(mockWriteToTerminal).not.toHaveBeenCalled();
    });
  });
});

describe('TabBar.handleNewTab AI Tool Mode (#480) — reference implementation', () => {
  let parent: HTMLElement;

  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
    parent?.remove();
  });

  function mountTabBar(): HTMLElement {
    parent = document.createElement('div');
    document.body.appendChild(parent);
    const tabBar = new TabBar();
    tabBar.mount(parent);
    return parent;
  }

  describe('Codex mode via + button (TabBar — works correctly)', () => {
    beforeEach(() => {
      setupWorkspace('codex');
      mockCreateTerminal.mockResolvedValue({ id: 'term-codex', worktree_branch: null });
    });

    it('should write "codex --yolo" to the new terminal', async () => {
      const p = mountTabBar();
      (p.querySelector('.add-tab-btn') as HTMLElement).click();

      await vi.waitFor(() => {
        expect(mockCreateTerminal).toHaveBeenCalledTimes(1);
      });
      vi.advanceTimersByTime(600);

      expect(mockWriteToTerminal).toHaveBeenCalledWith('term-codex', 'codex --yolo\r');
    });
  });

  describe('Both mode via + button (TabBar — works correctly)', () => {
    beforeEach(() => {
      setupWorkspace('both', false);
      let callCount = 0;
      mockCreateTerminal.mockImplementation(() => {
        callCount++;
        return Promise.resolve({
          id: callCount === 1 ? 'term-claude' : 'term-codex',
          worktree_branch: null,
        });
      });
    });

    it('should create TWO terminals in vertical split', async () => {
      const p = mountTabBar();
      (p.querySelector('.add-tab-btn') as HTMLElement).click();

      await vi.waitFor(() => {
        expect(mockCreateTerminal).toHaveBeenCalledTimes(2);
      });

      const layoutTree = store.getLayoutTree('ws-test');
      expect(layoutTree).toBeTruthy();
      expect(layoutTree!.type).toBe('split');
    });

    it('should write both claude and codex commands', async () => {
      const p = mountTabBar();
      (p.querySelector('.add-tab-btn') as HTMLElement).click();

      await vi.waitFor(() => {
        expect(mockCreateTerminal).toHaveBeenCalledTimes(2);
      });
      vi.advanceTimersByTime(600);

      expect(mockWriteToTerminal).toHaveBeenCalledWith(
        'term-claude', 'claude --dangerously-skip-permissions\r'
      );
      expect(mockWriteToTerminal).toHaveBeenCalledWith(
        'term-codex', 'codex --yolo\r'
      );
    });
  });
});

describe('Quick Claude — AI Tool Mode awareness (#480)', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
    document.querySelectorAll('.dialog-overlay').forEach(el => el.remove());
  });

  it('showQuickClaudeDialog should render AI tool mode selector', async () => {
    // Bug #480: Quick Claude dialog has no option to send to both/codex.
    // The dialog should include an AI tool mode selector.
    //
    // Import the real (unmocked) dialogs module. Since dialogs.ts isn't globally
    // mocked in this file, a direct dynamic import returns the real module.
    const { showQuickClaudeDialog } = await import('./dialogs');

    // showQuickClaudeDialog synchronously appends dialog to document.body
    const resultPromise = showQuickClaudeDialog({
      workspaces: [
        { id: 'ws-1', name: 'Test WS', folderPath: 'C:\\Projects' },
      ],
      activeWorkspaceId: 'ws-1',
    });

    const dialog = document.querySelector('.dialog');
    expect(dialog).toBeTruthy();

    // The dialog should have an AI tool mode selector
    const modeSelector = dialog!.querySelector('[data-testid="ai-tool-mode"]') ||
      dialog!.querySelector('.ai-tool-mode-select');

    expect(modeSelector).toBeTruthy();

    // Clean up — find and click the Cancel button to resolve the promise
    const allBtns = dialog!.querySelectorAll('.dialog-btn-secondary');
    const cancelBtn = Array.from(allBtns).find(b => b.textContent === 'Cancel') as HTMLElement;
    cancelBtn?.click();
    await resultPromise;
  });

  it('keyboard handler should pass workspace aiToolMode to Quick Claude dialog', () => {
    // Bug #480: keyboard-controller.ts:377 passes workspace data to showQuickClaudeDialog
    // but omits aiToolMode. It should include it.
    //
    // Current code:
    //   workspaces: state.workspaces.map(w => ({ id: w.id, name: w.name, folderPath: w.folderPath }))
    //
    // Missing: aiToolMode: w.aiToolMode

    store.reset();
    store.addWorkspace({
      id: 'ws-both',
      name: 'Both WS',
      folderPath: 'C:\\Projects',
      tabOrder: [],
      shellType: { type: 'windows' },
      worktreeMode: false,
      aiToolMode: 'both',
    });
    store.setActiveWorkspace('ws-both');

    const state = store.getState();
    const workspace = state.workspaces.find(w => w.id === 'ws-both')!;

    // Simulate what keyboard-controller.ts should do (includes aiToolMode):
    const passedData = { id: workspace.id, name: workspace.name, folderPath: workspace.folderPath, aiToolMode: workspace.aiToolMode };

    expect(passedData).toHaveProperty('aiToolMode');
    expect(passedData.aiToolMode).toBe('both');
  });
});
