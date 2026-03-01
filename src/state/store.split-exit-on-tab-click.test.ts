import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from './store';

// Bug #371: Clicking a tab while in split view keeps the split instead of
// exiting to single view. Expected: clicking a tab outside the split pair
// should clear the split and show the tab in normal single-pane mode.

describe('split view should exit when clicking a tab outside the split pair', () => {
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

  it('should clear split when clicking a tab outside the split pair', () => {
    // Bug #371: split [t1|t2], click t3 → should exit split, show t3 in single view
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Click t3 (outside split pair) — should clear split
    store.setActiveTerminal('t3');

    expect(store.getSplitView('ws-1')).toBeNull();
    expect(store.getState().activeTerminalId).toBe('t3');
  });

  it('should clear split when clicking a tab while right pane is focused', () => {
    // Bug #371: split [t1|t2] with right pane active, click t3 → exit split
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t2');

    store.setActiveTerminal('t3');

    expect(store.getSplitView('ws-1')).toBeNull();
    expect(store.getState().activeTerminalId).toBe('t3');
  });

  it('should not clear split when clicking a tab already in the split pair', () => {
    // Clicking a tab that's part of the split should just change focus, not clear
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t2');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
    expect(store.getState().activeTerminalId).toBe('t2');
  });

  it('should clear split after navigating through multiple non-split tabs', () => {
    // Bug #371: split [t1|t2], click t3 → exit split, click t4 → still no split
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    expect(store.getSplitView('ws-1')).toBeNull();

    store.setActiveTerminal('t4');
    expect(store.getSplitView('ws-1')).toBeNull();
    expect(store.getState().activeTerminalId).toBe('t4');
  });

  it('should restore split when navigating back to a former split terminal', () => {
    // Fix #426: split [t1|t2], click t3 → suspend split, click t1 → restore split
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    expect(store.getSplitView('ws-1')).toBeNull();

    // Navigate back to t1 — split should be restored
    store.setActiveTerminal('t1');
    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
    expect(store.getState().activeTerminalId).toBe('t1');
  });

  it('should clear vertical split when clicking an outside tab', () => {
    // Bug #371: same behavior for vertical splits
    store.setSplitView('ws-1', 't1', 't2', 'vertical', 0.7);
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');

    expect(store.getSplitView('ws-1')).toBeNull();
    expect(store.getState().activeTerminalId).toBe('t3');
  });
});
