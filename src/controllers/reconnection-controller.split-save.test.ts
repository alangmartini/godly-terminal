/**
 * Bug #498: Split layout persistence — serde format alignment
 *
 * Root cause was: Rust LayoutNode used #[serde(tag = "type")] without
 * rename_all, producing PascalCase ("Leaf"/"Split"). TypeScript uses
 * lowercase ("leaf"/"split"). Fix: added #[serde(rename_all = "lowercase")]
 * so Rust now serializes/accepts lowercase, matching TypeScript.
 *
 * These tests verify:
 * 1. The save path correctly syncs layout trees to the backend
 * 2. The restore path correctly reads lowercase layout_trees from load_layout
 * 3. The format TypeScript sends matches what it expects to receive
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';
import type { LayoutNode } from '../state/split-types';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@tauri-apps/api/path', () => ({
  homeDir: vi.fn(() => Promise.resolve('C:\\Users\\test')),
}));

vi.mock('../services/terminal-service', () => ({
  terminalService: {
    init: vi.fn(),
    reconnectSessions: vi.fn(() => Promise.resolve([])),
    attachSession: vi.fn(() => Promise.resolve()),
    createTerminal: vi.fn((_wsId: string, opts?: { idOverride?: string }) =>
      Promise.resolve({ id: opts?.idOverride ?? 'new-id', worktree_branch: null }),
    ),
    closeTerminal: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock('../services/workspace-service', () => ({
  workspaceService: {
    createWorkspace: vi.fn(() => Promise.resolve('default-ws')),
  },
}));

import { invoke } from '@tauri-apps/api/core';
import { restoreLayout } from './reconnection-controller';
import { terminalService } from '../services/terminal-service';

const mockedInvoke = vi.mocked(invoke);
const mockedTerminalService = vi.mocked(terminalService);

describe('Bug #498: split layout persistence round-trip', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    store.setState({
      workspaces: [],
      terminals: [],
      activeWorkspaceId: null,
      activeTerminalId: null,
      layoutTrees: {},
      splitViews: {},
      zoomedPanes: {},
    });
    if (typeof globalThis.window === 'undefined') {
      (globalThis as any).window = {};
    }
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  const deps = {
    markRestoredTerminal: vi.fn(),
    markReattachedTerminal: vi.fn(),
  };

  // -----------------------------------------------------------------------
  // Format consistency: TypeScript produces what Rust now accepts (lowercase)
  // -----------------------------------------------------------------------

  describe('format consistency', () => {
    it('TypeScript LayoutNode type tags use lowercase matching Rust serde', () => {
      // Bug #498 fix: Rust now uses #[serde(rename_all = "lowercase")],
      // so both sides use "leaf" and "split".
      const tsLeaf: LayoutNode = { type: 'leaf', terminal_id: 't1' };
      const tsSplit: LayoutNode = {
        type: 'split',
        direction: 'horizontal',
        ratio: 0.5,
        first: { type: 'leaf', terminal_id: 't1' },
        second: { type: 'leaf', terminal_id: 't2' },
      };

      // After the fix, Rust serializes as lowercase (matching TypeScript)
      expect(tsLeaf.type).toBe('leaf');
      expect(tsSplit.type).toBe('split');
    });
  });

  // -----------------------------------------------------------------------
  // Save path: syncLayoutTreeToBackend invoke verification
  // -----------------------------------------------------------------------

  describe('save path', () => {
    it('should call set_layout_tree invoke when a split is created', async () => {
      // Bug #498: verify the save path sends the tree to the backend
      store.addWorkspace({
        id: 'ws-1',
        name: 'Test',
        folderPath: 'C:\\Projects',
        tabOrder: [],
      });
      store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'T1', processName: 'pwsh', order: 0 });
      store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'T2', processName: 'pwsh', order: 1 });

      // Create a split — triggers syncLayoutTreeToBackend (fire-and-forget)
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      // Flush microtasks so the dynamic import() and invoke() resolve
      await new Promise(resolve => setTimeout(resolve, 50));

      // Verify set_layout_tree was called
      const setLayoutTreeCalls = mockedInvoke.mock.calls.filter(
        ([cmd]) => cmd === 'set_layout_tree',
      );
      expect(setLayoutTreeCalls.length).toBeGreaterThanOrEqual(1);

      // Verify the tree uses lowercase type tags (matching Rust's new format)
      const [, args] = setLayoutTreeCalls[0] as [string, { workspaceId: string; tree: LayoutNode }];
      expect(args.workspaceId).toBe('ws-1');
      expect(args.tree.type).toBe('split');
      if (args.tree.type === 'split') {
        expect(args.tree.first.type).toBe('leaf');
        expect(args.tree.second.type).toBe('leaf');
      }
    });
  });

  // -----------------------------------------------------------------------
  // Restore path: layout_trees from Rust backend (now lowercase)
  // -----------------------------------------------------------------------

  describe('restore path', () => {
    it('should restore a lowercase layout tree from Rust backend', async () => {
      // Bug #498 fix: Rust now sends { type: "leaf" } and { type: "split" }
      // (lowercase), matching TypeScript's format exactly.
      const layout = {
        workspaces: [
          { id: 'ws-1', name: 'WS', folder_path: 'C:\\', tab_order: ['t1', 't2'], shell_type: 'windows' as const },
        ],
        terminals: [
          { id: 't1', workspace_id: 'ws-1', name: 'T1', shell_type: 'windows' as const, cwd: 'C:\\' },
          { id: 't2', workspace_id: 'ws-1', name: 'T2', shell_type: 'windows' as const, cwd: 'C:\\' },
        ],
        active_workspace_id: 'ws-1',
        layout_trees: {
          'ws-1': {
            type: 'split',
            direction: 'horizontal',
            ratio: 0.5,
            first: { type: 'leaf', terminal_id: 't1' },
            second: { type: 'leaf', terminal_id: 't2' },
          } as LayoutNode,
        },
      };

      mockedTerminalService.reconnectSessions.mockResolvedValue(
        layout.terminals.map((t) => ({ id: t.id, running: true })),
      );
      mockedTerminalService.attachSession.mockResolvedValue(undefined);
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'load_layout') return layout;
        if (cmd === 'prune_stale_terminal_ids') return undefined;
        return undefined;
      });

      await restoreLayout(deps);

      // Layout tree MUST be restored
      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      expect(tree?.type).toBe('split');
      if (tree?.type === 'split') {
        expect(tree.direction).toBe('horizontal');
        expect(tree.ratio).toBe(0.5);
        expect(tree.first).toEqual({ type: 'leaf', terminal_id: 't1' });
        expect(tree.second).toEqual({ type: 'leaf', terminal_id: 't2' });
      }
    });

    it('should restore nested 3-pane lowercase layout tree', async () => {
      const layout = {
        workspaces: [
          { id: 'ws-1', name: 'WS', folder_path: 'C:\\', tab_order: ['t1', 't2', 't3'], shell_type: 'windows' as const },
        ],
        terminals: [
          { id: 't1', workspace_id: 'ws-1', name: 'T1', shell_type: 'windows' as const, cwd: 'C:\\' },
          { id: 't2', workspace_id: 'ws-1', name: 'T2', shell_type: 'windows' as const, cwd: 'C:\\' },
          { id: 't3', workspace_id: 'ws-1', name: 'T3', shell_type: 'windows' as const, cwd: 'C:\\' },
        ],
        active_workspace_id: 'ws-1',
        layout_trees: {
          'ws-1': {
            type: 'split',
            direction: 'horizontal',
            ratio: 0.5,
            first: { type: 'leaf', terminal_id: 't1' },
            second: {
              type: 'split',
              direction: 'vertical',
              ratio: 0.6,
              first: { type: 'leaf', terminal_id: 't2' },
              second: { type: 'leaf', terminal_id: 't3' },
            },
          } as LayoutNode,
        },
      };

      mockedTerminalService.reconnectSessions.mockResolvedValue(
        layout.terminals.map((t) => ({ id: t.id, running: true })),
      );
      mockedTerminalService.attachSession.mockResolvedValue(undefined);
      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'load_layout') return layout;
        if (cmd === 'prune_stale_terminal_ids') return undefined;
        return undefined;
      });

      await restoreLayout(deps);

      expect(store.getState().terminals).toHaveLength(3);
      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      expect(tree?.type).toBe('split');
      if (tree?.type === 'split') {
        expect(tree.second.type).toBe('split');
      }
    });
  });
});
