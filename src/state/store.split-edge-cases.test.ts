import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from './store';

// Comprehensive edge-case tests for split view behavior.
// Covers close-in-split, move-to-workspace, workspace switching,
// overwrite-split, and single-tab scenarios.

describe('split view edge cases', () => {
  const ws1: Workspace = {
    id: 'ws-1', name: 'WS 1', folderPath: 'C:\\ws1', tabOrder: [],
    shellType: { type: 'windows' }, worktreeMode: false, aiToolMode: 'none',
  };
  const ws2: Workspace = {
    id: 'ws-2', name: 'WS 2', folderPath: 'C:\\ws2', tabOrder: [],
    shellType: { type: 'windows' }, worktreeMode: false, aiToolMode: 'none',
  };

  beforeEach(() => {
    store.reset();
    store.addWorkspace(ws1);
    store.addWorkspace(ws2);
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 1 });
    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 2 });
    store.addTerminal({ id: 't4', workspaceId: 'ws-2', name: 'Tab 4', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't5', workspaceId: 'ws-2', name: 'Tab 5', processName: 'cmd', order: 1 });
    store.setActiveWorkspace('ws-1');
  });

  describe('close terminal while in split', () => {
    it('should clear split and activate partner when active pane is closed', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.removeTerminal('t1');

      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t2');
    });

    it('should clear split and activate partner when non-active pane is closed', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.removeTerminal('t2');

      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t1');
    });

    it('should not affect split when closing a non-split terminal', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.removeTerminal('t3');

      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();
      expect(split!.leftTerminalId).toBe('t1');
      expect(split!.rightTerminalId).toBe('t2');
    });

    it('should handle closing both split terminals sequentially', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.removeTerminal('t1'); // clears split, activates t2
      expect(store.getSplitView('ws-1')).toBeNull();

      store.removeTerminal('t2'); // t3 becomes active
      expect(store.getState().activeTerminalId).toBe('t3');
    });
  });

  describe('move terminal to another workspace while in split', () => {
    it('should clear split when left pane is moved', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.moveTerminalToWorkspace('t1', 'ws-2');

      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should clear split when right pane is moved', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.moveTerminalToWorkspace('t2', 'ws-2');

      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should not affect split when moving a non-split terminal', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.moveTerminalToWorkspace('t3', 'ws-2');

      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();
      expect(split!.leftTerminalId).toBe('t1');
      expect(split!.rightTerminalId).toBe('t2');
    });

    it('should not create a split in the target workspace', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.moveTerminalToWorkspace('t1', 'ws-2');

      expect(store.getSplitView('ws-2')).toBeNull();
    });
  });

  describe('workspace switching with split', () => {
    it('should preserve split when switching away and back', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.setActiveWorkspace('ws-2');
      store.setActiveWorkspace('ws-1');

      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();
      expect(split!.leftTerminalId).toBe('t1');
      expect(split!.rightTerminalId).toBe('t2');
    });

    it('should allow independent splits in different workspaces', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.setActiveWorkspace('ws-2');
      store.setSplitView('ws-2', 't4', 't5', 'vertical');

      expect(store.getSplitView('ws-1')).not.toBeNull();
      expect(store.getSplitView('ws-2')).not.toBeNull();
      expect(store.getSplitView('ws-1')!.direction).toBe('horizontal');
      expect(store.getSplitView('ws-2')!.direction).toBe('vertical');
    });

    it('should restore correct active terminal in split after workspace switch', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t2'); // focus right pane

      store.setActiveWorkspace('ws-2');
      store.setActiveWorkspace('ws-1');

      // Should remember that t2 was the last active terminal
      expect(store.getState().activeTerminalId).toBe('t2');
    });
  });

  describe('overwrite existing split', () => {
    it('should replace split when creating a new one in the same workspace', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.setSplitView('ws-1', 't2', 't3', 'vertical');

      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();
      expect(split!.leftTerminalId).toBe('t2');
      expect(split!.rightTerminalId).toBe('t3');
      expect(split!.direction).toBe('vertical');
    });

    it('should not affect other workspace splits when overwriting', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveWorkspace('ws-2');
      store.setSplitView('ws-2', 't4', 't5', 'vertical');

      store.setActiveWorkspace('ws-1');
      store.setSplitView('ws-1', 't1', 't3', 'horizontal');

      // ws-2 split should be untouched
      const ws2Split = store.getSplitView('ws-2');
      expect(ws2Split).not.toBeNull();
      expect(ws2Split!.leftTerminalId).toBe('t4');
    });
  });

  describe('workspace deletion with split', () => {
    it('should clean up split when workspace is deleted', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveWorkspace('ws-2');

      store.removeWorkspace('ws-1');

      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should not affect other workspace splits', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setSplitView('ws-2', 't4', 't5', 'vertical');
      store.setActiveWorkspace('ws-2');

      store.removeWorkspace('ws-1');

      const ws2Split = store.getSplitView('ws-2');
      expect(ws2Split).not.toBeNull();
      expect(ws2Split!.leftTerminalId).toBe('t4');
    });
  });

  describe('split with only 2 tabs in workspace', () => {
    it('should leave only 1 tab after closing a split pane in a 2-tab workspace', () => {
      // Remove t3 so ws-1 has only t1, t2
      store.removeTerminal('t3');
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.removeTerminal('t1');

      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t2');
      // Only 1 terminal left in ws-1
      const wsTerminals = store.getWorkspaceTerminals('ws-1');
      expect(wsTerminals.length).toBe(1);
    });
  });

  describe('split ratio edge cases', () => {
    it('should default to 0.5 ratio', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      expect(store.getSplitView('ws-1')!.ratio).toBe(0.5);
    });

    it('should preserve custom ratio', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal', 0.3);
      expect(store.getSplitView('ws-1')!.ratio).toBe(0.3);
    });

    it('should update ratio via updateSplitRatio', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.updateSplitRatio('ws-1', 0.7);
      expect(store.getSplitView('ws-1')!.ratio).toBe(0.7);
    });

    it('should be a no-op when updating ratio for non-existent split', () => {
      store.updateSplitRatio('ws-1', 0.7);
      expect(store.getSplitView('ws-1')).toBeNull();
    });
  });

  describe('focus other pane', () => {
    it('should switch focus between split panes without clearing split', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      // Simulate split.focusOtherPane: switch to the other split terminal
      const split = store.getSplitView('ws-1')!;
      const otherId = store.getState().activeTerminalId === split.leftTerminalId
        ? split.rightTerminalId
        : split.leftTerminalId;
      store.setActiveTerminal(otherId);

      expect(store.getSplitView('ws-1')).not.toBeNull();
      expect(store.getState().activeTerminalId).toBe('t2');
    });

    it('should toggle focus back and forth', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      store.setActiveTerminal('t2');
      store.setActiveTerminal('t1');
      store.setActiveTerminal('t2');

      expect(store.getSplitView('ws-1')).not.toBeNull();
      expect(store.getState().activeTerminalId).toBe('t2');
    });
  });

  describe('clearSplitView idempotency', () => {
    it('should be safe to clear a split that does not exist', () => {
      store.clearSplitView('ws-1');
      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should be safe to clear the same split twice', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.clearSplitView('ws-1');
      store.clearSplitView('ws-1');
      expect(store.getSplitView('ws-1')).toBeNull();
    });
  });
});
