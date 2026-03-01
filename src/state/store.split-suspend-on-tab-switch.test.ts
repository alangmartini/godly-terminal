import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from './store';

// Bug #426: Split view is permanently lost when switching to a tab outside the
// split pair and then switching back. Expected: the split should be suspended
// (not destroyed) and restored when navigating back to a terminal that was in it.

describe('split view should be preserved when switching tabs and coming back', () => {
  const ws1: Workspace = {
    id: 'ws-1', name: 'WS 1', folderPath: 'C:\\ws1', tabOrder: [],
    shellType: { type: 'windows' }, worktreeMode: false, aiToolMode: 'none',
  };

  beforeEach(() => {
    store.reset();
    store.addWorkspace(ws1);
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't4', workspaceId: 'ws-1', name: 'Tab 4', processName: 'cmd', order: 0 });
    store.setActiveWorkspace('ws-1');
  });

  it('should restore split when navigating back to left terminal after switching away', () => {
    // Bug #426: split [t1|t2], click t3, click t1 → split should be restored
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Navigate away to t3
    store.setActiveTerminal('t3');

    // Navigate back to t1 — split should be restored
    store.setActiveTerminal('t1');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should restore split when navigating back to right terminal after switching away', () => {
    // Bug #426: split [t1|t2], click t3, click t2 → split should be restored
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t2');

    // Navigate away to t3
    store.setActiveTerminal('t3');

    // Navigate back to t2 — split should be restored
    store.setActiveTerminal('t2');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should restore split after visiting multiple non-split tabs', () => {
    // Bug #426: split [t1|t2], click t3, click t4, click t1 → still restores
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t4');

    // Come back to t1 — split should be restored
    store.setActiveTerminal('t1');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should preserve split ratio after tab switch round-trip', () => {
    // Bug #426: custom ratio should survive the suspension
    store.setSplitView('ws-1', 't1', 't2', 'horizontal', 0.7);
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t1');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.ratio).toBe(0.7);
  });

  it('should preserve split direction after tab switch round-trip', () => {
    // Bug #426: vertical split should survive the suspension
    store.setSplitView('ws-1', 't1', 't2', 'vertical');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t2');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.direction).toBe('vertical');
  });

  it('should preserve layout tree (not just legacy split) after tab switch', () => {
    // Bug #426: the modern layoutTrees state should be preserved too
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t1');

    const tree = store.getLayoutTree('ws-1');
    expect(tree).not.toBeNull();
    expect(tree!.type).toBe('split');
  });
});
