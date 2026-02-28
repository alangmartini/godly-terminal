import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from './store';

// Navigating to a tab outside the split pair suspends the split.
// Navigating back to a terminal in the split restores it.

describe('split view suspend and restore on navigation', () => {
  const ws1: Workspace = {
    id: 'ws-1', name: 'WS 1', folderPath: 'C:\\ws1', tabOrder: [],
    shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
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

  it('should restore split when navigating back to the left terminal', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Navigate away suspends split
    store.setActiveTerminal('t3');
    expect(store.getSplitView('ws-1')).toBeNull();

    // Navigate back to t1 — split is restored
    store.setActiveTerminal('t1');
    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should restore split when navigating back to the right terminal', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Navigate to t3 suspends split
    store.setActiveTerminal('t3');
    expect(store.getSplitView('ws-1')).toBeNull();

    // Navigate to t2 — split is restored
    store.setActiveTerminal('t2');
    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
    expect(store.getState().activeTerminalId).toBe('t2');
  });

  it('should not affect switching within the split pair', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Switch to t2 (in split) — split stays
    store.setActiveTerminal('t2');
    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should restore split after visiting multiple non-split tabs', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // First outside tab suspends split
    store.setActiveTerminal('t3');
    expect(store.getSplitView('ws-1')).toBeNull();

    // Subsequent non-split navigation keeps split suspended
    store.setActiveTerminal('t4');
    expect(store.getSplitView('ws-1')).toBeNull();

    // Navigate back to t2 — split is restored
    store.setActiveTerminal('t2');
    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should clear split if one of the split terminals was closed', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.removeTerminal('t2');
    store.setActiveTerminal('t1');

    expect(store.getSplitView('ws-1')).toBeNull();
  });

  it('should clear split if both split terminals were closed', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.removeTerminal('t1');
    store.removeTerminal('t2');
    store.setActiveTerminal('t4');

    expect(store.getSplitView('ws-1')).toBeNull();
  });

  it('should allow creating a new split after clearing via navigation', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Navigate away clears split
    store.setActiveTerminal('t3');
    expect(store.getSplitView('ws-1')).toBeNull();

    // Create a new split
    store.setSplitView('ws-1', 't3', 't4', 'vertical');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t3');
    expect(split!.rightTerminalId).toBe('t4');
    expect(split!.direction).toBe('vertical');
  });
});
