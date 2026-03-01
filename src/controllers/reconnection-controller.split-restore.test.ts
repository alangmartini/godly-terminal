/**
 * Bug #498: Split layouts lost on app restart
 *
 * When restarting Godly Terminal (the Tauri app, not the daemon), all split
 * pane layouts are lost. Terminals that were in split views revert to
 * single-pane/tabbed view.
 *
 * Root cause: restoreLayout() in reconnection-controller.ts doesn't read
 * `layout_trees` from the load_layout response — it only reads the legacy
 * `split_views` field. Even when the backend returns layout_trees (e.g.,
 * persisted from MCP-created splits), the frontend ignores them.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';
import type { LayoutNode } from '../state/split-types';

// Mock Tauri APIs before importing the module under test
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@tauri-apps/api/path', () => ({
  homeDir: vi.fn(() => Promise.resolve('C:\\Users\\test')),
}));

// Mock terminal service
vi.mock('../services/terminal-service', () => ({
  terminalService: {
    init: vi.fn(),
    reconnectSessions: vi.fn(() => Promise.resolve([])),
    attachSession: vi.fn(() => Promise.resolve()),
    createTerminal: vi.fn((workspaceId: string, opts?: { idOverride?: string }) =>
      Promise.resolve({ id: opts?.idOverride ?? 'new-id', worktree_branch: null }),
    ),
    closeTerminal: vi.fn(() => Promise.resolve()),
  },
}));

// Import after mocks are set up
import { invoke } from '@tauri-apps/api/core';
import { restoreLayout } from './reconnection-controller';
import { terminalService } from '../services/terminal-service';

const mockedInvoke = vi.mocked(invoke);
const mockedTerminalService = vi.mocked(terminalService);

describe('Bug #498: Split layout restoration on restart', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store to clean state
    store.setState({
      workspaces: [],
      terminals: [],
      activeWorkspaceId: null,
      activeTerminalId: null,
      layoutTrees: {},
      splitViews: {},
      zoomedPanes: {},
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  /**
   * Helper: build a layout response with both terminals and layout_trees.
   * This is what the Rust backend returns from load_layout when
   * layout_trees are persisted (e.g., MCP-created splits).
   */
  function buildLayoutWithLayoutTrees(tree: LayoutNode) {
    return {
      workspaces: [
        {
          id: 'ws-1',
          name: 'Test Workspace',
          folder_path: 'C:\\Projects',
          tab_order: ['term-1', 'term-2'],
          shell_type: 'windows' as const,
        },
      ],
      terminals: [
        {
          id: 'term-1',
          workspace_id: 'ws-1',
          name: 'Terminal 1',
          shell_type: 'windows' as const,
          cwd: 'C:\\Projects',
        },
        {
          id: 'term-2',
          workspace_id: 'ws-1',
          name: 'Terminal 2',
          shell_type: 'windows' as const,
          cwd: 'C:\\Projects',
        },
      ],
      active_workspace_id: 'ws-1',
      // The key field — layout_trees is returned by the Rust backend
      // but the frontend's TypeScript type annotation doesn't include it
      layout_trees: {
        'ws-1': tree,
      },
    };
  }

  function setupMocks(layout: ReturnType<typeof buildLayoutWithLayoutTrees>) {
    // Mock all sessions as alive in daemon (reattach path)
    mockedTerminalService.reconnectSessions.mockResolvedValue(
      layout.terminals.map((t) => ({ id: t.id, running: true })),
    );
    mockedTerminalService.attachSession.mockResolvedValue(undefined);

    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'load_layout') return layout;
      if (cmd === 'prune_stale_terminal_ids') return undefined;
      if (cmd === 'set_split_view') return undefined;
      return undefined;
    });
  }

  const deps = {
    markRestoredTerminal: vi.fn(),
    markReattachedTerminal: vi.fn(),
  };

  it('should restore a simple 2-pane horizontal split from layout_trees', async () => {
    // Bug #498: layout_trees returned by backend but frontend ignores them
    const tree: LayoutNode = {
      type: 'split',
      direction: 'horizontal',
      ratio: 0.5,
      first: { type: 'leaf', terminal_id: 'term-1' },
      second: { type: 'leaf', terminal_id: 'term-2' },
    };

    const layout = buildLayoutWithLayoutTrees(tree);
    setupMocks(layout);

    await restoreLayout(deps);

    // Verify terminals were restored
    const state = store.getState();
    expect(state.terminals).toHaveLength(2);
    expect(state.terminals.map((t) => t.id).sort()).toEqual(['term-1', 'term-2']);

    // Verify layout tree was restored — this is the actual bug assertion.
    // The store should have a layout tree for ws-1 that matches the saved tree.
    const restoredTree = store.getLayoutTree('ws-1');
    expect(restoredTree).not.toBeNull();
    expect(restoredTree).not.toBeUndefined();
    expect(restoredTree?.type).toBe('split');
    if (restoredTree?.type === 'split') {
      expect(restoredTree.direction).toBe('horizontal');
      expect(restoredTree.ratio).toBe(0.5);
      expect(restoredTree.first).toEqual({ type: 'leaf', terminal_id: 'term-1' });
      expect(restoredTree.second).toEqual({ type: 'leaf', terminal_id: 'term-2' });
    }
  });

  it('should restore a vertical split from layout_trees', async () => {
    // Bug #498: vertical splits via layout_trees also lost
    const tree: LayoutNode = {
      type: 'split',
      direction: 'vertical',
      ratio: 0.6,
      first: { type: 'leaf', terminal_id: 'term-1' },
      second: { type: 'leaf', terminal_id: 'term-2' },
    };

    const layout = buildLayoutWithLayoutTrees(tree);
    setupMocks(layout);

    await restoreLayout(deps);

    const restoredTree = store.getLayoutTree('ws-1');
    expect(restoredTree).not.toBeNull();
    if (restoredTree?.type === 'split') {
      expect(restoredTree.direction).toBe('vertical');
      expect(restoredTree.ratio).toBe(0.6);
    }
  });

  it('should restore nested 3-pane split from layout_trees', async () => {
    // Bug #498: nested splits can't be represented by legacy split_views at all
    const tree: LayoutNode = {
      type: 'split',
      direction: 'horizontal',
      ratio: 0.5,
      first: { type: 'leaf', terminal_id: 'term-1' },
      second: {
        type: 'split',
        direction: 'vertical',
        ratio: 0.5,
        first: { type: 'leaf', terminal_id: 'term-2' },
        second: { type: 'leaf', terminal_id: 'term-3' },
      },
    };

    // Add a third terminal
    const layout = {
      ...buildLayoutWithLayoutTrees(tree),
      terminals: [
        { id: 'term-1', workspace_id: 'ws-1', name: 'Terminal 1', shell_type: 'windows' as const, cwd: 'C:\\Projects' },
        { id: 'term-2', workspace_id: 'ws-1', name: 'Terminal 2', shell_type: 'windows' as const, cwd: 'C:\\Projects' },
        { id: 'term-3', workspace_id: 'ws-1', name: 'Terminal 3', shell_type: 'windows' as const, cwd: 'C:\\Projects' },
      ],
    };
    layout.workspaces[0].tab_order = ['term-1', 'term-2', 'term-3'];
    layout.layout_trees = { 'ws-1': tree };
    setupMocks(layout);

    await restoreLayout(deps);

    const restoredTree = store.getLayoutTree('ws-1');
    expect(restoredTree).not.toBeNull();
    expect(restoredTree?.type).toBe('split');
    if (restoredTree?.type === 'split') {
      expect(restoredTree.first).toEqual({ type: 'leaf', terminal_id: 'term-1' });
      expect(restoredTree.second.type).toBe('split');
      if (restoredTree.second.type === 'split') {
        expect(restoredTree.second.direction).toBe('vertical');
        expect(restoredTree.second.first).toEqual({ type: 'leaf', terminal_id: 'term-2' });
        expect(restoredTree.second.second).toEqual({ type: 'leaf', terminal_id: 'term-3' });
      }
    }
  });

  it('should prefer layout_trees over split_views when both are present', async () => {
    // Bug #498: When the backend returns both layout_trees and split_views,
    // the frontend should use layout_trees (the newer, richer format)
    const tree: LayoutNode = {
      type: 'split',
      direction: 'horizontal',
      ratio: 0.7,
      first: { type: 'leaf', terminal_id: 'term-1' },
      second: { type: 'leaf', terminal_id: 'term-2' },
    };

    const layout = {
      ...buildLayoutWithLayoutTrees(tree),
      // Legacy split_views with DIFFERENT ratio (0.3 vs 0.7)
      split_views: {
        'ws-1': {
          left_terminal_id: 'term-1',
          right_terminal_id: 'term-2',
          direction: 'horizontal',
          ratio: 0.3,
        },
      },
    };
    setupMocks(layout);

    await restoreLayout(deps);

    const restoredTree = store.getLayoutTree('ws-1');
    expect(restoredTree).not.toBeNull();
    if (restoredTree?.type === 'split') {
      // Should use the layout_trees ratio (0.7), not split_views ratio (0.3)
      expect(restoredTree.ratio).toBe(0.7);
    }
  });

  it('should fall back to split_views when layout_trees is empty', async () => {
    // Backward compatibility: old layouts only have split_views
    const layout = {
      workspaces: [
        {
          id: 'ws-1',
          name: 'Test Workspace',
          folder_path: 'C:\\Projects',
          tab_order: ['term-1', 'term-2'],
          shell_type: 'windows' as const,
        },
      ],
      terminals: [
        { id: 'term-1', workspace_id: 'ws-1', name: 'Terminal 1', shell_type: 'windows' as const, cwd: 'C:\\Projects' },
        { id: 'term-2', workspace_id: 'ws-1', name: 'Terminal 2', shell_type: 'windows' as const, cwd: 'C:\\Projects' },
      ],
      active_workspace_id: 'ws-1',
      split_views: {
        'ws-1': {
          left_terminal_id: 'term-1',
          right_terminal_id: 'term-2',
          direction: 'horizontal',
          ratio: 0.5,
        },
      },
    };

    mockedTerminalService.reconnectSessions.mockResolvedValue(
      layout.terminals.map((t) => ({ id: t.id, running: true })),
    );
    mockedTerminalService.attachSession.mockResolvedValue(undefined);
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'load_layout') return layout;
      if (cmd === 'prune_stale_terminal_ids') return undefined;
      if (cmd === 'set_split_view') return undefined;
      return undefined;
    });

    await restoreLayout(deps);

    // Fallback: split_views should still be restored (existing behavior)
    const restoredTree = store.getLayoutTree('ws-1');
    expect(restoredTree).not.toBeNull();
    if (restoredTree?.type === 'split') {
      expect(restoredTree.direction).toBe('horizontal');
      expect(restoredTree.ratio).toBe(0.5);
    }
  });

  it('should handle multiple workspaces with different layout_trees', async () => {
    // Bug #498: each workspace's split layout should be independently restored
    const tree1: LayoutNode = {
      type: 'split',
      direction: 'horizontal',
      ratio: 0.5,
      first: { type: 'leaf', terminal_id: 'term-1' },
      second: { type: 'leaf', terminal_id: 'term-2' },
    };
    const tree2: LayoutNode = {
      type: 'split',
      direction: 'vertical',
      ratio: 0.6,
      first: { type: 'leaf', terminal_id: 'term-3' },
      second: { type: 'leaf', terminal_id: 'term-4' },
    };

    const layout = {
      workspaces: [
        { id: 'ws-1', name: 'WS 1', folder_path: 'C:\\A', tab_order: ['term-1', 'term-2'], shell_type: 'windows' as const },
        { id: 'ws-2', name: 'WS 2', folder_path: 'C:\\B', tab_order: ['term-3', 'term-4'], shell_type: 'windows' as const },
      ],
      terminals: [
        { id: 'term-1', workspace_id: 'ws-1', name: 'T1', shell_type: 'windows' as const, cwd: 'C:\\A' },
        { id: 'term-2', workspace_id: 'ws-1', name: 'T2', shell_type: 'windows' as const, cwd: 'C:\\A' },
        { id: 'term-3', workspace_id: 'ws-2', name: 'T3', shell_type: 'windows' as const, cwd: 'C:\\B' },
        { id: 'term-4', workspace_id: 'ws-2', name: 'T4', shell_type: 'windows' as const, cwd: 'C:\\B' },
      ],
      active_workspace_id: 'ws-1',
      layout_trees: {
        'ws-1': tree1,
        'ws-2': tree2,
      },
    };

    mockedTerminalService.reconnectSessions.mockResolvedValue(
      layout.terminals.map((t) => ({ id: t.id, running: true })),
    );
    mockedTerminalService.attachSession.mockResolvedValue(undefined);
    mockedInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'load_layout') return layout;
      if (cmd === 'prune_stale_terminal_ids') return undefined;
      if (cmd === 'set_split_view') return undefined;
      return undefined;
    });

    await restoreLayout(deps);

    const ws1Tree = store.getLayoutTree('ws-1');
    const ws2Tree = store.getLayoutTree('ws-2');

    expect(ws1Tree).not.toBeNull();
    expect(ws2Tree).not.toBeNull();

    if (ws1Tree?.type === 'split') {
      expect(ws1Tree.direction).toBe('horizontal');
    }
    if (ws2Tree?.type === 'split') {
      expect(ws2Tree.direction).toBe('vertical');
      expect(ws2Tree.ratio).toBe(0.6);
    }
  });
});
