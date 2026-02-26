import { describe, it, expect, beforeEach, vi } from 'vitest';
import { store, Workspace } from './store';
import { terminalIds } from './split-types';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

const ws: Workspace = {
  id: 'ws-1', name: 'WS', folderPath: 'C:\\ws', tabOrder: [],
  shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
};

function addTerminals(ids: string[]) {
  for (let i = 0; i < ids.length; i++) {
    store.addTerminal({
      id: ids[i], workspaceId: 'ws-1', name: `Tab ${i + 1}`,
      processName: 'cmd', order: i,
    });
  }
}

describe('layout tree state management', () => {
  beforeEach(() => {
    store.reset();
    store.addWorkspace(ws);
  });

  // -------------------------------------------------------------------------
  // Creating trees from splitTerminalAt
  // -------------------------------------------------------------------------

  describe('splitTerminalAt', () => {
    it('should create a tree from an empty workspace', () => {
      addTerminals(['t1', 't2']);
      store.setActiveWorkspace('ws-1');

      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      expect(tree!.type).toBe('split');
      if (tree!.type === 'split') {
        expect(tree!.direction).toBe('horizontal');
        expect(tree!.ratio).toBe(0.5);
        expect(tree!.first).toEqual({ type: 'leaf', terminal_id: 't1' });
        expect(tree!.second).toEqual({ type: 'leaf', terminal_id: 't2' });
      }
    });

    it('should create a tree with custom ratio', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'vertical', 0.3);

      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      if (tree!.type === 'split') {
        expect(tree!.direction).toBe('vertical');
        expect(tree!.ratio).toBe(0.3);
      }
    });

    it('should nest a split within an existing tree', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      // Now split t2 vertically with t3
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      expect(tree!.type).toBe('split');
      if (tree!.type === 'split') {
        expect(tree!.first).toEqual({ type: 'leaf', terminal_id: 't1' });
        expect(tree!.second.type).toBe('split');
        if (tree!.second.type === 'split') {
          expect(tree!.second.direction).toBe('vertical');
          expect(tree!.second.first).toEqual({ type: 'leaf', terminal_id: 't2' });
          expect(tree!.second.second).toEqual({ type: 'leaf', terminal_id: 't3' });
        }
      }
    });

    it('should collect all terminal IDs from nested tree', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      const tree = store.getLayoutTree('ws-1');
      expect(terminalIds(tree!)).toEqual(['t1', 't2', 't3']);
    });
  });

  // -------------------------------------------------------------------------
  // Removing terminals from trees
  // -------------------------------------------------------------------------

  describe('unsplitTerminal', () => {
    it('should collapse a 2-pane tree to nothing', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.unsplitTerminal('ws-1', 't2');

      expect(store.getLayoutTree('ws-1')).toBeNull();
    });

    it('should collapse a 3-pane tree to a 2-pane tree', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      // Remove t3 — the nested split should collapse
      store.unsplitTerminal('ws-1', 't3');

      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      expect(tree!.type).toBe('split');
      if (tree!.type === 'split') {
        expect(tree!.first).toEqual({ type: 'leaf', terminal_id: 't1' });
        expect(tree!.second).toEqual({ type: 'leaf', terminal_id: 't2' });
      }
    });

    it('should clear tree when removing all but one terminal', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      store.unsplitTerminal('ws-1', 't3');
      store.unsplitTerminal('ws-1', 't2');

      expect(store.getLayoutTree('ws-1')).toBeNull();
    });

    it('should no-op for terminal not in tree', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.unsplitTerminal('ws-1', 't3');

      // Tree should be unchanged
      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      expect(terminalIds(tree!)).toEqual(['t1', 't2']);
    });

    it('should remove the entire tree when called without terminalId', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.unsplitTerminal('ws-1');

      expect(store.getLayoutTree('ws-1')).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // removeTerminal integration
  // -------------------------------------------------------------------------

  describe('removeTerminal with layout tree', () => {
    it('should clear tree when removing a terminal from a 2-pane split', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.removeTerminal('t1');

      expect(store.getLayoutTree('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t2');
    });

    it('should collapse tree when removing from a 3-pane split', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');
      store.setActiveTerminal('t3');

      store.removeTerminal('t3');

      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      if (tree) {
        expect(terminalIds(tree)).toEqual(['t1', 't2']);
      }
    });

    it('should set remaining terminal as active when tree collapses', () => {
      addTerminals(['t1', 't2']);
      store.setActiveWorkspace('ws-1');
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t2');

      store.removeTerminal('t2');

      expect(store.getState().activeTerminalId).toBe('t1');
    });
  });

  // -------------------------------------------------------------------------
  // Zoom
  // -------------------------------------------------------------------------

  describe('zoom', () => {
    it('should set zoomed pane', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.setZoomedPane('ws-1', 't1');

      expect(store.getZoomedPane('ws-1')).toBe('t1');
    });

    it('should unzoom with null', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setZoomedPane('ws-1', 't1');

      store.setZoomedPane('ws-1', null);

      expect(store.getZoomedPane('ws-1')).toBeNull();
    });

    it('should preserve tree when zoomed', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setZoomedPane('ws-1', 't1');

      // Tree should still exist
      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      expect(terminalIds(tree!)).toEqual(['t1', 't2']);
    });

    it('should restore tree layout on unzoom', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal', 0.7);
      store.setZoomedPane('ws-1', 't1');
      store.setZoomedPane('ws-1', null);

      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      if (tree!.type === 'split') {
        expect(tree!.ratio).toBe(0.7);
      }
    });

    it('should clear zoom when removing zoomed terminal', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');
      store.setZoomedPane('ws-1', 't2');

      store.removeTerminal('t2');

      expect(store.getZoomedPane('ws-1')).toBeNull();
    });

    it('should clear zoom when clearing tree', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setZoomedPane('ws-1', 't1');

      store.clearLayoutTree('ws-1');

      expect(store.getZoomedPane('ws-1')).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // Swap panes
  // -------------------------------------------------------------------------

  describe('swapPanes', () => {
    it('should swap two terminals in a 2-pane tree', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.swapPanes('ws-1', 't1', 't2');

      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      if (tree!.type === 'split') {
        expect(tree!.first).toEqual({ type: 'leaf', terminal_id: 't2' });
        expect(tree!.second).toEqual({ type: 'leaf', terminal_id: 't1' });
      }
    });

    it('should swap terminals in a nested tree', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      store.swapPanes('ws-1', 't1', 't3');

      const tree = store.getLayoutTree('ws-1');
      expect(terminalIds(tree!)).toEqual(['t3', 't2', 't1']);
    });

    it('should no-op when swapping non-existent terminal', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.swapPanes('ws-1', 't1', 'nonexistent');

      const tree = store.getLayoutTree('ws-1');
      expect(terminalIds(tree!)).toEqual(['t1', 't2']);
    });

    it('should no-op when no tree exists', () => {
      addTerminals(['t1', 't2']);
      store.swapPanes('ws-1', 't1', 't2');

      expect(store.getLayoutTree('ws-1')).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // getAdjacentPane
  // -------------------------------------------------------------------------

  describe('getAdjacentPane', () => {
    it('should find adjacent pane in horizontal split', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      expect(store.getAdjacentPane('ws-1', 't1', 'horizontal', true)).toBe('t2');
      expect(store.getAdjacentPane('ws-1', 't2', 'horizontal', false)).toBe('t1');
    });

    it('should find adjacent pane in vertical split', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'vertical');

      expect(store.getAdjacentPane('ws-1', 't1', 'vertical', true)).toBe('t2');
      expect(store.getAdjacentPane('ws-1', 't2', 'vertical', false)).toBe('t1');
    });

    it('should return null when no adjacent in that direction', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      // Looking vertically in a horizontal split should find nothing
      expect(store.getAdjacentPane('ws-1', 't1', 'vertical', true)).toBeNull();
    });

    it('should navigate nested splits correctly', () => {
      addTerminals(['t1', 't2', 't3']);
      // Structure: t1 | (t2 / t3) — horizontal at root, vertical in second child
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      // From t1, go right → should reach t2 (first leaf of the right subtree)
      expect(store.getAdjacentPane('ws-1', 't1', 'horizontal', true)).toBe('t2');
      // From t2, go down → should reach t3
      expect(store.getAdjacentPane('ws-1', 't2', 'vertical', true)).toBe('t3');
      // From t3, go up → should reach t2
      expect(store.getAdjacentPane('ws-1', 't3', 'vertical', false)).toBe('t2');
      // From t2, go left → should reach t1
      expect(store.getAdjacentPane('ws-1', 't2', 'horizontal', false)).toBe('t1');
    });

    it('should return null when no tree', () => {
      addTerminals(['t1']);
      expect(store.getAdjacentPane('ws-1', 't1', 'horizontal', true)).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // updateTreeRatio and updateLayoutTreeRatio
  // -------------------------------------------------------------------------

  describe('updateTreeRatio', () => {
    it('should update root ratio', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.updateTreeRatio('ws-1', [], 0.3);

      const tree = store.getLayoutTree('ws-1');
      if (tree!.type === 'split') {
        expect(tree!.ratio).toBe(0.3);
      }
    });

    it('should clamp ratio to minimum 0.15', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.updateTreeRatio('ws-1', [], 0.05);

      const tree = store.getLayoutTree('ws-1');
      if (tree!.type === 'split') {
        expect(tree!.ratio).toBe(0.15);
      }
    });

    it('should clamp ratio to maximum 0.85', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.updateTreeRatio('ws-1', [], 0.95);

      const tree = store.getLayoutTree('ws-1');
      if (tree!.type === 'split') {
        expect(tree!.ratio).toBe(0.85);
      }
    });

    it('should update nested split ratio via path', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      // Path [1] → second child of root (the nested vertical split)
      store.updateTreeRatio('ws-1', [1], 0.7);

      const tree = store.getLayoutTree('ws-1');
      if (tree!.type === 'split' && tree!.second.type === 'split') {
        expect(tree!.second.ratio).toBe(0.7);
        // Root ratio should be unchanged
        expect(tree!.ratio).toBe(0.5);
      }
    });

    it('should no-op for invalid path', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal', 0.5);

      store.updateTreeRatio('ws-1', [0], 0.3); // path [0] points to a leaf

      const tree = store.getLayoutTree('ws-1');
      if (tree!.type === 'split') {
        expect(tree!.ratio).toBe(0.5); // unchanged
      }
    });
  });

  describe('updateLayoutTreeRatio', () => {
    it('should delegate to updateTreeRatio', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.updateLayoutTreeRatio('ws-1', [], 0.7);

      const tree = store.getLayoutTree('ws-1');
      if (tree!.type === 'split') {
        expect(tree!.ratio).toBe(0.7);
      }
    });

    it('should update ratio at nested level', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      store.updateLayoutTreeRatio('ws-1', [1], 0.3);

      const tree = store.getLayoutTree('ws-1');
      if (tree!.type === 'split' && tree!.second.type === 'split') {
        expect(tree!.second.ratio).toBe(0.3);
        expect(tree!.ratio).toBe(0.5);
      }
    });

    it('should no-op when workspace has no tree', () => {
      store.updateLayoutTreeRatio('ws-1', [], 0.7);
      expect(store.getLayoutTree('ws-1')).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // getFocusedPaneId
  // -------------------------------------------------------------------------

  describe('getFocusedPaneId', () => {
    it('should return active terminal if in tree', () => {
      addTerminals(['t1', 't2']);
      store.setActiveWorkspace('ws-1');
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      expect(store.getFocusedPaneId('ws-1')).toBe('t1');
    });

    it('should return null if active terminal not in tree', () => {
      addTerminals(['t1', 't2', 't3']);
      store.setActiveWorkspace('ws-1');
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      // setActiveTerminal('t3') will clear the tree since t3 is outside it.
      // Use setState to set active terminal without triggering tree logic.
      store.setState({ activeTerminalId: 't3' });

      expect(store.getFocusedPaneId('ws-1')).toBeNull();
    });

    it('should return null when no tree', () => {
      addTerminals(['t1']);
      store.setActiveWorkspace('ws-1');
      store.setActiveTerminal('t1');

      expect(store.getFocusedPaneId('ws-1')).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // syncSessionPauseState with tree
  // -------------------------------------------------------------------------

  describe('syncSessionPauseState with layout tree', () => {
    it('should resume all tree terminals and pause non-tree terminals', async () => {
      const { invoke } = await import('@tauri-apps/api/core') as { invoke: ReturnType<typeof vi.fn> };

      addTerminals(['t1', 't2', 't3', 't4']);
      store.setActiveWorkspace('ws-1');
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');
      store.setActiveTerminal('t1');

      // After setup, clear mocks and trigger a fresh sync
      invoke.mockClear();

      // Force t4 into the resumed set by simulating it was previously resumed
      // Then sync should pause it since it's not visible
      invoke('resume_session', { sessionId: 't4' });
      invoke.mockClear();

      store.syncSessionPauseState();

      // After sync: t1, t2, t3 are in tree so already resumed (no new calls).
      // The important check: no resume call for t4 (it's not in tree or active)
      const resumeCalls = invoke.mock.calls.filter(
        (c: unknown[]) => c[0] === 'resume_session'
      );
      const resumedIds = resumeCalls.map(
        (c: unknown[]) => (c[1] as { sessionId: string }).sessionId
      );
      expect(resumedIds).not.toContain('t4');
    });

    it('should include all tree terminals as visible', () => {
      addTerminals(['t1', 't2', 't3', 't4']);
      store.setActiveWorkspace('ws-1');

      // Create a 3-pane tree
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');
      store.setActiveTerminal('t1');

      // All tree terminals should be visible (in the layout tree)
      const tree = store.getLayoutTree('ws-1');
      expect(tree).not.toBeNull();
      expect(terminalIds(tree!)).toEqual(['t1', 't2', 't3']);
    });
  });

  // -------------------------------------------------------------------------
  // Tab adjacency with layout tree
  // -------------------------------------------------------------------------

  describe('tab adjacency with layout tree', () => {
    it('should order tabs to match depth-first tree traversal', () => {
      addTerminals(['t1', 't2', 't3', 't4']);
      store.setActiveWorkspace('ws-1');

      // Create tree: t1 | (t3 / t2) — t3 and t2 are not adjacent initially
      store.splitTerminalAt('ws-1', 't1', 't3', 'horizontal');
      store.splitTerminalAt('ws-1', 't3', 't2', 'vertical');

      const terminals = store.getWorkspaceTerminals('ws-1');
      const ids = terminals.map(t => t.id);
      const t1Idx = ids.indexOf('t1');
      const t3Idx = ids.indexOf('t3');
      const t2Idx = ids.indexOf('t2');

      // DFS order: t1, t3, t2 — they should be adjacent
      expect(t3Idx).toBe(t1Idx + 1);
      expect(t2Idx).toBe(t3Idx + 1);
    });

    it('should reorder tabs when split is created with non-adjacent terminals', () => {
      addTerminals(['t1', 't2', 't3', 't4']);
      store.setActiveWorkspace('ws-1');

      // Split t1 with t4 (non-adjacent in original order)
      store.splitTerminalAt('ws-1', 't1', 't4', 'horizontal');

      const terminals = store.getWorkspaceTerminals('ws-1');
      const ids = terminals.map(t => t.id);
      const t1Idx = ids.indexOf('t1');
      const t4Idx = ids.indexOf('t4');

      expect(Math.abs(t1Idx - t4Idx)).toBe(1);
    });
  });

  // -------------------------------------------------------------------------
  // Legacy getSplitView wrapper
  // -------------------------------------------------------------------------

  describe('legacy getSplitView wrapper', () => {
    it('should return correct data for 2-pane trees', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal', 0.6);

      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();
      expect(split!.leftTerminalId).toBe('t1');
      expect(split!.rightTerminalId).toBe('t2');
      expect(split!.direction).toBe('horizontal');
      expect(split!.ratio).toBe(0.6);
    });

    it('should return null for 3+ pane trees', () => {
      addTerminals(['t1', 't2', 't3']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should return null when no tree exists', () => {
      addTerminals(['t1']);
      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should work with setSplitView creating a tree', () => {
      addTerminals(['t1', 't2']);
      store.setSplitView('ws-1', 't1', 't2', 'vertical', 0.7);

      // Both tree and legacy API should work
      expect(store.getLayoutTree('ws-1')).not.toBeNull();
      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();
      expect(split!.direction).toBe('vertical');
      expect(split!.ratio).toBe(0.7);
    });
  });

  // -------------------------------------------------------------------------
  // setActiveTerminal with tree
  // -------------------------------------------------------------------------

  describe('setActiveTerminal with layout tree', () => {
    it('should not clear tree when clicking a terminal in the tree', () => {
      addTerminals(['t1', 't2']);
      store.setActiveWorkspace('ws-1');
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.setActiveTerminal('t2');

      expect(store.getLayoutTree('ws-1')).not.toBeNull();
      expect(store.getState().activeTerminalId).toBe('t2');
    });

    it('should clear tree when clicking a terminal outside the tree', () => {
      addTerminals(['t1', 't2', 't3']);
      store.setActiveWorkspace('ws-1');
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      // Click t3 (not in tree) — should clear the tree and show single-pane mode
      store.setActiveTerminal('t3');

      expect(store.getLayoutTree('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t3');
    });
  });

  // -------------------------------------------------------------------------
  // clearLayoutTree
  // -------------------------------------------------------------------------

  describe('clearLayoutTree', () => {
    it('should clear tree and splitViews', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.clearLayoutTree('ws-1');

      expect(store.getLayoutTree('ws-1')).toBeNull();
      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should clear zoom when clearing tree', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setZoomedPane('ws-1', 't1');

      store.clearLayoutTree('ws-1');

      expect(store.getZoomedPane('ws-1')).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // setLayoutTree and getLayoutTree
  // -------------------------------------------------------------------------

  describe('setLayoutTree / getLayoutTree', () => {
    it('should store and retrieve tree', () => {
      addTerminals(['t1', 't2']);

      const tree = {
        type: 'split' as const,
        direction: 'horizontal' as const,
        ratio: 0.4,
        first: { type: 'leaf' as const, terminal_id: 't1' },
        second: { type: 'leaf' as const, terminal_id: 't2' },
      };
      store.setLayoutTree('ws-1', tree);

      const retrieved = store.getLayoutTree('ws-1');
      expect(retrieved).toEqual(tree);
    });

    it('should return null for workspace without tree', () => {
      expect(store.getLayoutTree('ws-1')).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // reset clears all tree state
  // -------------------------------------------------------------------------

  describe('reset', () => {
    it('should clear layoutTrees and zoomedPanes on reset', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setZoomedPane('ws-1', 't1');

      store.reset();

      expect(store.getState().layoutTrees).toEqual({});
      expect(store.getState().zoomedPanes).toEqual({});
      expect(store.getState().splitViews).toEqual({});
    });
  });

  // -------------------------------------------------------------------------
  // removeWorkspace clears tree state
  // -------------------------------------------------------------------------

  describe('removeWorkspace', () => {
    it('should clean up tree state when removing workspace', () => {
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
      store.setZoomedPane('ws-1', 't1');

      store.removeWorkspace('ws-1');

      expect(store.getLayoutTree('ws-1')).toBeNull();
      expect(store.getZoomedPane('ws-1')).toBeNull();
      expect(store.getSplitView('ws-1')).toBeNull();
    });
  });

  // -------------------------------------------------------------------------
  // moveTerminalToWorkspace with tree
  // -------------------------------------------------------------------------

  describe('moveTerminalToWorkspace with tree', () => {
    it('should clear tree when moving a terminal from a 2-pane split', () => {
      store.addWorkspace({
        id: 'ws-2', name: 'WS 2', folderPath: 'C:\\ws2', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
      });
      addTerminals(['t1', 't2']);
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

      store.moveTerminalToWorkspace('t1', 'ws-2');

      expect(store.getLayoutTree('ws-1')).toBeNull();
    });
  });
});
