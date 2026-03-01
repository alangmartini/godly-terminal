import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from './store';

// Bug #309: Split panel tabs should be grouped in tab bar.
// When in split mode, tab navigation (nextTab/prevTab) breaks the split
// because it navigates to individual tabs rather than treating the split
// pair as a group. Also, reordering one split tab doesn't keep its
// partner adjacent.

describe('split tab grouping (#309)', () => {
  const ws: Workspace = {
    id: 'ws-1', name: 'WS', folderPath: 'C:\\ws', tabOrder: [],
    shellType: { type: 'windows' }, worktreeMode: false, aiToolMode: 'none',
  };

  beforeEach(() => {
    store.reset();
    store.addWorkspace(ws);
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 1 });
    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 2 });
    store.addTerminal({ id: 't4', workspaceId: 'ws-1', name: 'Tab 4', processName: 'cmd', order: 3 });
    store.setActiveWorkspace('ws-1');
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');
  });

  describe('tab navigation clears split when leaving the pair', () => {
    // Navigating (Ctrl+Tab) to a tab outside the split pair clears the split,
    // just like clicking a tab. Feature #329 may later add grouped navigation.

    it('nextTab within split pair should preserve split', () => {
      const terminals = store.getWorkspaceTerminals('ws-1');
      const currentIndex = terminals.findIndex(t => t.id === 't1');
      const nextIndex = (currentIndex + 1) % terminals.length;
      store.setActiveTerminal(terminals[nextIndex].id); // t1→t2 (within split)

      expect(store.getSplitView('ws-1')).not.toBeNull();
    });

    it('nextTab outside split pair should clear split', () => {
      // t1→t2 (within split), then t2→t3 (outside) clears split
      store.setActiveTerminal('t2');
      expect(store.getSplitView('ws-1')).not.toBeNull();

      const terminals = store.getWorkspaceTerminals('ws-1');
      const idx = terminals.findIndex(t => t.id === 't2');
      const nextIdx = (idx + 1) % terminals.length;
      store.setActiveTerminal(terminals[nextIdx].id); // goes to t3

      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t3');
    });

    it('prevTab outside split pair should clear split', () => {
      store.setActiveTerminal('t1');
      const terminals = store.getWorkspaceTerminals('ws-1');
      const idx = terminals.findIndex(t => t.id === 't1');
      const prevIdx = (idx - 1 + terminals.length) % terminals.length;
      store.setActiveTerminal(terminals[prevIdx].id); // wraps to t4

      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t4');
    });
  });

  describe('split tabs should stay adjacent after reorder', () => {
    // Bug #309: Dragging one split tab to a distant position separates
    // the pair. The partner tab should follow to maintain adjacency.

    it('reordering left split tab should keep right tab adjacent', () => {
      // Tab order: t1, t2, t3, t4. Split: t1|t2
      // Drag t1 to position after t3: new order should be t3, t1, t2, t4
      // (t2 follows t1 to maintain adjacency)
      store.reorderTerminals('ws-1', ['t3', 't1', 't4', 't2']);

      // After reorder, split tabs should be adjacent
      const terminals = store.getWorkspaceTerminals('ws-1');
      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();

      const leftIdx = terminals.findIndex(t => t.id === split!.leftTerminalId);
      const rightIdx = terminals.findIndex(t => t.id === split!.rightTerminalId);

      // Bug: t1 is at index 1, t2 is at index 3 — they're separated
      // Expected: they should be adjacent (|leftIdx - rightIdx| === 1)
      expect(Math.abs(leftIdx - rightIdx)).toBe(1);
    });

    it('reordering right split tab should keep left tab adjacent', () => {
      // Tab order: t1, t2, t3, t4. Split: t1|t2
      // Drag t2 to position after t4: new order should be t1, t3, t4, t2
      // But with grouping, t1 should follow: t3, t4, t1, t2
      store.reorderTerminals('ws-1', ['t1', 't3', 't4', 't2']);

      const terminals = store.getWorkspaceTerminals('ws-1');
      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();

      const leftIdx = terminals.findIndex(t => t.id === split!.leftTerminalId);
      const rightIdx = terminals.findIndex(t => t.id === split!.rightTerminalId);

      expect(Math.abs(leftIdx - rightIdx)).toBe(1);
    });
  });

  describe('split tabs visual grouping', () => {
    // Bug #309: Split tabs should be visually grouped or merged in the tab bar.
    // At the store level, this means split terminals should always be adjacent
    // in the tab order.

    it('creating a split should make the two terminals adjacent in tab order', () => {
      // Reset and set up non-adjacent tabs
      store.reset();
      store.addWorkspace(ws);
      store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 1 });
      store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 2 });
      store.addTerminal({ id: 't4', workspaceId: 'ws-1', name: 'Tab 4', processName: 'cmd', order: 3 });
      store.setActiveWorkspace('ws-1');

      // Create split between non-adjacent tabs: t1 and t3
      store.setSplitView('ws-1', 't1', 't3', 'horizontal');
      store.setActiveTerminal('t1');

      const terminals = store.getWorkspaceTerminals('ws-1');
      const leftIdx = terminals.findIndex(t => t.id === 't1');
      const rightIdx = terminals.findIndex(t => t.id === 't3');

      // Bug: t1 is at index 0, t3 is at index 2 — not adjacent
      // Expected: creating a split should reorder so the pair is adjacent
      expect(Math.abs(leftIdx - rightIdx)).toBe(1);
    });

    it('split terminals should be adjacent even when other tabs exist between them', () => {
      // Tab order: t1, t2, t3, t4. Split: t1|t2 (already adjacent by default)
      // Add a background terminal — the split pair should remain adjacent
      // (foreground addTerminal clears the split per Bug #391)
      store.addTerminal({ id: 't5', workspaceId: 'ws-1', name: 'Tab 5', processName: 'cmd', order: 1 }, { background: true });

      const terminals = store.getWorkspaceTerminals('ws-1');
      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();

      const leftIdx = terminals.findIndex(t => t.id === split!.leftTerminalId);
      const rightIdx = terminals.findIndex(t => t.id === split!.rightTerminalId);

      expect(Math.abs(leftIdx - rightIdx)).toBe(1);
    });
  });
});
