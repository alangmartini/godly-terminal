import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from './store';

// Bug: split view is permanently destroyed when switching to a tab outside the split.
// Switching back to either split terminal should restore the split view.

describe('split view preservation across tab switches', () => {
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

  it('should restore split when switching back to the left terminal', () => {
    // Bug: split between t1|t2, switch to t3, switch back to t1 → split gone
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Navigate away to a terminal outside the split
    store.setActiveTerminal('t3');

    // Navigate back to the left split terminal
    store.setActiveTerminal('t1');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should restore split when switching back to the right terminal', () => {
    // Bug: split between t1|t2, switch to t3, switch back to t2 → split gone
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t2');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should preserve split direction and ratio after round-trip', () => {
    // Bug: even if the split is "restored", direction/ratio could be lost
    store.setSplitView('ws-1', 't1', 't2', 'vertical', 0.7);
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t1');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.direction).toBe('vertical');
    expect(split!.ratio).toBe(0.7);
  });

  it('should restore split after visiting multiple non-split tabs', () => {
    // Bug: navigating through several tabs before returning should still restore
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t4');
    store.setActiveTerminal('t3');
    store.setActiveTerminal('t2');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should not restore split if one of the split terminals was closed', () => {
    // Edge case: if t2 was removed while viewing t3, the split cannot be restored
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.removeTerminal('t2');
    store.setActiveTerminal('t1');

    expect(store.getSplitView('ws-1')).toBeNull();
  });

  it('should not restore split if both split terminals were closed', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.removeTerminal('t1');
    store.removeTerminal('t2');
    store.setActiveTerminal('t3');

    expect(store.getSplitView('ws-1')).toBeNull();
  });

  it('should allow creating a new split after the previous one was dismissed and restored', () => {
    // Create a split, navigate away and back (restoring it), then explicitly clear
    // and create a different split — the new split should take priority
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t1');

    // Now explicitly clear and create a new split
    store.clearSplitView('ws-1');
    store.setSplitView('ws-1', 't3', 't4', 'vertical');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t3');
    expect(split!.rightTerminalId).toBe('t4');
    expect(split!.direction).toBe('vertical');
  });
});
