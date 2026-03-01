import { describe, it, expect, beforeEach, vi } from 'vitest';
import { store, Workspace } from './store';
import { containsTerminal, terminalIds } from './split-types';

// Bug #391: Creating a new terminal (Ctrl+T) during an active split does not
// clear the layout tree. addTerminal() bypasses setActiveTerminal(), leaving
// the tree intact while activeTerminalId points outside it — an inconsistent
// state where the split renders stale panes.

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

const ws: Workspace = {
  id: 'ws-1', name: 'WS', folderPath: 'C:\\ws', tabOrder: [],
  shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
};

describe('Bug #391: new terminal during active split creates inconsistent state', () => {
  beforeEach(() => {
    store.reset();
    store.addWorkspace(ws);
  });

  it('should clear layout tree when addTerminal makes a non-tree terminal active', () => {
    // Bug #391: split [t1|t2], add t3 via addTerminal (simulates Ctrl+T)
    // addTerminal sets activeTerminalId = t3 directly, but t3 is not in the tree.
    // Expected: layout tree should be cleared since the active terminal is outside it.
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

    // Simulate Ctrl+T: createNewTerminal() calls addTerminal without background flag
    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });

    // After adding t3, activeTerminalId should be t3
    expect(store.getState().activeTerminalId).toBe('t3');
    // The layout tree should be cleared since the active terminal is not in it
    expect(store.getLayoutTree('ws-1')).toBeNull();
  });

  it('should not leave activeTerminalId pointing outside the layout tree', () => {
    // Bug #391: The core invariant violation — activeTerminalId must either be
    // in the layout tree or the tree must be null.
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });

    const tree = store.getLayoutTree('ws-1');
    const activeId = store.getState().activeTerminalId;

    // Invariant: if a tree exists, the active terminal must be in it
    if (tree) {
      expect(containsTerminal(tree, activeId!)).toBe(true);
    }
  });

  it('should preserve split when addTerminal is called with background flag', () => {
    // Background terminals should not affect the active terminal or the split
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

    // Background add should not change active terminal or clear tree
    store.addTerminal(
      { id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 },
      { background: true }
    );

    expect(store.getState().activeTerminalId).toBe('t1');
    const tree = store.getLayoutTree('ws-1');
    expect(tree).not.toBeNull();
    expect(terminalIds(tree!)).toEqual(['t1', 't2']);
  });

  it('should suspend split when adding a new terminal, and restore on navigation back (Bug #426)', () => {
    // Bug #426 supersedes Bug #391 part 2: addTerminal now suspends the split
    // so it can be restored when the user navigates back to a split member.
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');

    // Add t3 (simulates Ctrl+T) — should clear the active split but suspend it
    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });

    // Click t3 tab
    store.setActiveTerminal('t3');
    expect(store.getLayoutTree('ws-1')).toBeNull();
    expect(store.getState().activeTerminalId).toBe('t3');

    // Click back on t2 — split should be restored
    store.setActiveTerminal('t2');
    expect(store.getLayoutTree('ws-1')).not.toBeNull();
    expect(store.getState().activeTerminalId).toBe('t2');

    // Click back on t1 — split should still be active
    store.setActiveTerminal('t1');
    expect(store.getLayoutTree('ws-1')).not.toBeNull();
    expect(store.getState().activeTerminalId).toBe('t1');
  });

  it('should clear vertical split when new terminal is added', () => {
    // Same bug for vertical splits
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'vertical', 0.7);

    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });

    expect(store.getState().activeTerminalId).toBe('t3');
    expect(store.getLayoutTree('ws-1')).toBeNull();
    expect(store.getSplitView('ws-1')).toBeNull();
  });

  it('should clear nested 3-pane split when new terminal is added', () => {
    // Bug #391 with a more complex tree: t1 | (t2 / t3)
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
    store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');

    // Add t4 (not in the 3-pane tree)
    store.addTerminal({ id: 't4', workspaceId: 'ws-1', name: 'Tab 4', processName: 'cmd', order: 0 });

    expect(store.getState().activeTerminalId).toBe('t4');
    expect(store.getLayoutTree('ws-1')).toBeNull();
  });
});
