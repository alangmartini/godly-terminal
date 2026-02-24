import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from './store';

// Split view is updated in-place when navigating to a tab outside the split.
// The active pane is replaced with the new terminal, keeping the split alive.

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

  it('should update split when navigating away and back to the left terminal', () => {
    // Split t1|t2, navigate to t3 (replaces active left pane), navigate back to t1
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Navigate away: active=t1 (left), so left pane becomes t3 → split [t3|t2]
    store.setActiveTerminal('t3');

    // Navigate back to t1: active=t3 (left), so left pane becomes t1 → split [t1|t2]
    store.setActiveTerminal('t1');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t1');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should update split when navigating away then to the right terminal', () => {
    // Split t1|t2, navigate to t3 (replaces left), then click t2 (already in split)
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Navigate to t3: active=t1 (left) → split becomes [t3|t2]
    store.setActiveTerminal('t3');
    // Navigate to t2: t2 is in split (right), just change focus
    store.setActiveTerminal('t2');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t3');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should preserve split direction and ratio after round-trip', () => {
    store.setSplitView('ws-1', 't1', 't2', 'vertical', 0.7);
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t1');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.direction).toBe('vertical');
    expect(split!.ratio).toBe(0.7);
  });

  it('should keep split active after visiting multiple non-split tabs', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Navigate through several tabs — split updates each time but never disappears
    store.setActiveTerminal('t3'); // [t3|t2]
    store.setActiveTerminal('t4'); // [t4|t2]
    store.setActiveTerminal('t3'); // [t3|t2]
    store.setActiveTerminal('t2'); // t2 is in split, just focus

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t3');
    expect(split!.rightTerminalId).toBe('t2');
  });

  it('should clear split if one of the split terminals was closed', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3'); // split becomes [t3|t2]
    store.removeTerminal('t2');    // t2 is in split → split cleared
    store.setActiveTerminal('t1');

    expect(store.getSplitView('ws-1')).toBeNull();
  });

  it('should clear split if both split terminals were closed', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3'); // split becomes [t3|t2]
    store.removeTerminal('t3');    // t3 is in split (left) → split cleared
    store.removeTerminal('t2');
    store.setActiveTerminal('t4');

    expect(store.getSplitView('ws-1')).toBeNull();
  });

  it('should allow creating a new split after clearing and navigating', () => {
    store.setSplitView('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    store.setActiveTerminal('t3');
    store.setActiveTerminal('t1');

    // Explicitly clear and create a new split
    store.clearSplitView('ws-1');
    store.setSplitView('ws-1', 't3', 't4', 'vertical');

    const split = store.getSplitView('ws-1');
    expect(split).not.toBeNull();
    expect(split!.leftTerminalId).toBe('t3');
    expect(split!.rightTerminalId).toBe('t4');
    expect(split!.direction).toBe('vertical');
  });
});
