import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace, Terminal } from './store';

describe('Store', () => {
  beforeEach(() => {
    store.reset();
  });

  describe('workspace operations', () => {
    it('should add workspace with windows shell type', () => {
      const workspace: Workspace = {
        id: 'ws-1',
        name: 'Test Workspace',
        folderPath: 'C:\\Users\\test',
        tabOrder: [],
        shellType: { type: 'windows' },
      };

      store.addWorkspace(workspace);

      const state = store.getState();
      expect(state.workspaces).toHaveLength(1);
      expect(state.workspaces[0].shellType).toEqual({ type: 'windows' });
    });

    it('should add workspace with wsl shell type without distribution', () => {
      const workspace: Workspace = {
        id: 'ws-2',
        name: 'WSL Workspace',
        folderPath: '/home/user',
        tabOrder: [],
        shellType: { type: 'wsl' },
      };

      store.addWorkspace(workspace);

      const state = store.getState();
      expect(state.workspaces).toHaveLength(1);
      expect(state.workspaces[0].shellType).toEqual({ type: 'wsl' });
      expect(state.workspaces[0].shellType.type).toBe('wsl');
      if (state.workspaces[0].shellType.type === 'wsl') {
        expect(state.workspaces[0].shellType.distribution).toBeUndefined();
      }
    });

    it('should add workspace with wsl shell type with distribution', () => {
      const workspace: Workspace = {
        id: 'ws-3',
        name: 'Ubuntu Workspace',
        folderPath: '/home/user/project',
        tabOrder: [],
        shellType: { type: 'wsl', distribution: 'Ubuntu' },
      };

      store.addWorkspace(workspace);

      const state = store.getState();
      expect(state.workspaces).toHaveLength(1);
      const shellType = state.workspaces[0].shellType;
      expect(shellType.type).toBe('wsl');
      if (shellType.type === 'wsl') {
        expect(shellType.distribution).toBe('Ubuntu');
      }
    });

    it('should update workspace shell type from windows to wsl', () => {
      const workspace: Workspace = {
        id: 'ws-4',
        name: 'Changing Workspace',
        folderPath: 'C:\\Users\\test',
        tabOrder: [],
        shellType: { type: 'windows' },
      };

      store.addWorkspace(workspace);
      store.updateWorkspace('ws-4', {
        shellType: { type: 'wsl', distribution: 'Debian' },
      });

      const state = store.getState();
      const updated = state.workspaces.find(w => w.id === 'ws-4');
      expect(updated?.shellType.type).toBe('wsl');
      if (updated?.shellType.type === 'wsl') {
        expect(updated.shellType.distribution).toBe('Debian');
      }
    });

    it('should update workspace shell type from wsl to windows', () => {
      const workspace: Workspace = {
        id: 'ws-5',
        name: 'WSL to Windows',
        folderPath: '/home/user',
        tabOrder: [],
        shellType: { type: 'wsl', distribution: 'Ubuntu' },
      };

      store.addWorkspace(workspace);
      store.updateWorkspace('ws-5', {
        shellType: { type: 'windows' },
      });

      const state = store.getState();
      const updated = state.workspaces.find(w => w.id === 'ws-5');
      expect(updated?.shellType).toEqual({ type: 'windows' });
    });

    it('should reorder workspaces by id list', () => {
      store.addWorkspace({
        id: 'ws-a', name: 'A', folderPath: 'C:\\a', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false,
      });
      store.addWorkspace({
        id: 'ws-b', name: 'B', folderPath: 'C:\\b', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false,
      });
      store.addWorkspace({
        id: 'ws-c', name: 'C', folderPath: 'C:\\c', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false,
      });

      store.reorderWorkspaces(['ws-c', 'ws-a', 'ws-b']);

      const state = store.getState();
      expect(state.workspaces.map(w => w.id)).toEqual(['ws-c', 'ws-a', 'ws-b']);
    });

    it('should ignore unknown ids in reorderWorkspaces', () => {
      store.addWorkspace({
        id: 'ws-a', name: 'A', folderPath: 'C:\\a', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false,
      });
      store.addWorkspace({
        id: 'ws-b', name: 'B', folderPath: 'C:\\b', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false,
      });

      store.reorderWorkspaces(['ws-b', 'ws-nonexistent', 'ws-a']);

      const state = store.getState();
      expect(state.workspaces.map(w => w.id)).toEqual(['ws-b', 'ws-a']);
    });

    it('should preserve workspace data after reorder', () => {
      store.addWorkspace({
        id: 'ws-a', name: 'Alpha', folderPath: 'C:\\alpha', tabOrder: ['t1'],
        shellType: { type: 'wsl', distribution: 'Ubuntu' }, worktreeMode: true,
      });
      store.addWorkspace({
        id: 'ws-b', name: 'Beta', folderPath: 'C:\\beta', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false,
      });

      store.reorderWorkspaces(['ws-b', 'ws-a']);

      const state = store.getState();
      const alpha = state.workspaces.find(w => w.id === 'ws-a');
      expect(alpha?.name).toBe('Alpha');
      expect(alpha?.folderPath).toBe('C:\\alpha');
      expect(alpha?.shellType).toEqual({ type: 'wsl', distribution: 'Ubuntu' });
      expect(alpha?.worktreeMode).toBe(true);
    });

    it('should remove workspace and its terminals', () => {
      const workspace: Workspace = {
        id: 'ws-6',
        name: 'To Remove',
        folderPath: 'C:\\temp',
        tabOrder: [],
        shellType: { type: 'windows' },
      };

      const terminal: Terminal = {
        id: 'term-1',
        workspaceId: 'ws-6',
        name: 'Shell',
        processName: 'powershell',
        order: 0,
      };

      store.addWorkspace(workspace);
      store.addTerminal(terminal);
      store.removeWorkspace('ws-6');

      const state = store.getState();
      expect(state.workspaces).toHaveLength(0);
      expect(state.terminals).toHaveLength(0);
    });
  });

  describe('terminal operations', () => {
    it('should add terminal and set it as active', () => {
      const workspace: Workspace = {
        id: 'ws-1',
        name: 'Test',
        folderPath: 'C:\\',
        tabOrder: [],
        shellType: { type: 'windows' },
      };

      const terminal: Terminal = {
        id: 'term-1',
        workspaceId: 'ws-1',
        name: 'PowerShell',
        processName: 'powershell.exe',
        order: 0,
      };

      store.addWorkspace(workspace);
      store.addTerminal(terminal);

      const state = store.getState();
      expect(state.terminals).toHaveLength(1);
      expect(state.activeTerminalId).toBe('term-1');
    });

    it('should get workspace terminals sorted by order', () => {
      const workspace: Workspace = {
        id: 'ws-1',
        name: 'Test',
        folderPath: 'C:\\',
        tabOrder: [],
        shellType: { type: 'windows' },
      };

      store.addWorkspace(workspace);

      // Add terminals in order - store assigns order based on count
      store.addTerminal({
        id: 'term-1',
        workspaceId: 'ws-1',
        name: 'First',
        processName: 'cmd',
        order: 0,
      });
      store.addTerminal({
        id: 'term-2',
        workspaceId: 'ws-1',
        name: 'Second',
        processName: 'cmd',
        order: 0,
      });
      store.addTerminal({
        id: 'term-3',
        workspaceId: 'ws-1',
        name: 'Third',
        processName: 'cmd',
        order: 0,
      });

      // Store recalculates order based on addition sequence
      const terminals = store.getWorkspaceTerminals('ws-1');
      expect(terminals).toHaveLength(3);
      expect(terminals[0].id).toBe('term-1');
      expect(terminals[1].id).toBe('term-2');
      expect(terminals[2].id).toBe('term-3');
    });
  });

  describe('oscTitle and userRenamed fields', () => {
    it('should store oscTitle via updateTerminal', () => {
      store.addWorkspace({
        id: 'ws-1', name: 'WS', folderPath: 'C:\\', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
      });
      store.addTerminal({
        id: 't-1', workspaceId: 'ws-1', name: 'Terminal', processName: 'powershell', order: 0,
      });

      store.updateTerminal('t-1', { oscTitle: 'vim README.md' });

      const t = store.getState().terminals.find(t => t.id === 't-1');
      expect(t?.oscTitle).toBe('vim README.md');
    });

    it('should clear oscTitle by setting undefined', () => {
      store.addWorkspace({
        id: 'ws-1', name: 'WS', folderPath: 'C:\\', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
      });
      store.addTerminal({
        id: 't-1', workspaceId: 'ws-1', name: 'Terminal', processName: 'powershell', order: 0,
      });

      store.updateTerminal('t-1', { oscTitle: 'vim' });
      store.updateTerminal('t-1', { oscTitle: undefined });

      const t = store.getState().terminals.find(t => t.id === 't-1');
      expect(t?.oscTitle).toBeUndefined();
    });

    it('should store userRenamed via updateTerminal', () => {
      store.addWorkspace({
        id: 'ws-1', name: 'WS', folderPath: 'C:\\', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
      });
      store.addTerminal({
        id: 't-1', workspaceId: 'ws-1', name: 'Terminal', processName: 'powershell', order: 0,
      });

      store.updateTerminal('t-1', { userRenamed: true });

      const t = store.getState().terminals.find(t => t.id === 't-1');
      expect(t?.userRenamed).toBe(true);
    });

    it('should default oscTitle and userRenamed to undefined on new terminals', () => {
      store.addWorkspace({
        id: 'ws-1', name: 'WS', folderPath: 'C:\\', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
      });
      store.addTerminal({
        id: 't-1', workspaceId: 'ws-1', name: 'Terminal', processName: 'powershell', order: 0,
      });

      const t = store.getState().terminals.find(t => t.id === 't-1');
      expect(t?.oscTitle).toBeUndefined();
      expect(t?.userRenamed).toBeUndefined();
    });
  });

  describe('claude code mode', () => {
    it('should store claudeCodeMode on workspace', () => {
      const workspace: Workspace = {
        id: 'ws-cc',
        name: 'Claude Code Workspace',
        folderPath: 'C:\\Projects',
        tabOrder: [],
        shellType: { type: 'windows' },
        worktreeMode: false,
        claudeCodeMode: true,
      };

      store.addWorkspace(workspace);

      const state = store.getState();
      expect(state.workspaces[0].claudeCodeMode).toBe(true);
    });

    it('should toggle claudeCodeMode via updateWorkspace', () => {
      store.addWorkspace({
        id: 'ws-cc-toggle',
        name: 'Toggle Test',
        folderPath: 'C:\\test',
        tabOrder: [],
        shellType: { type: 'windows' },
        worktreeMode: false,
        claudeCodeMode: false,
      });

      expect(store.getState().workspaces[0].claudeCodeMode).toBe(false);

      store.updateWorkspace('ws-cc-toggle', { claudeCodeMode: true });

      expect(store.getState().workspaces[0].claudeCodeMode).toBe(true);

      store.updateWorkspace('ws-cc-toggle', { claudeCodeMode: false });

      expect(store.getState().workspaces[0].claudeCodeMode).toBe(false);
    });

    it('should not affect other workspaces when toggling claudeCodeMode', () => {
      store.addWorkspace({
        id: 'ws-a',
        name: 'A',
        folderPath: 'C:\\a',
        tabOrder: [],
        shellType: { type: 'windows' },
        worktreeMode: false,
        claudeCodeMode: false,
      });
      store.addWorkspace({
        id: 'ws-b',
        name: 'B',
        folderPath: 'C:\\b',
        tabOrder: [],
        shellType: { type: 'windows' },
        worktreeMode: false,
        claudeCodeMode: false,
      });

      store.updateWorkspace('ws-a', { claudeCodeMode: true });

      const state = store.getState();
      expect(state.workspaces.find(w => w.id === 'ws-a')?.claudeCodeMode).toBe(true);
      expect(state.workspaces.find(w => w.id === 'ws-b')?.claudeCodeMode).toBe(false);
    });
  });

  describe('active tab memory across workspace switches', () => {
    // Bug: switching to another workspace and back resets tab to the first one
    // instead of restoring the previously active tab
    const ws1: Workspace = {
      id: 'ws-1', name: 'WS 1', folderPath: 'C:\\ws1', tabOrder: [],
      shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
    };
    const ws2: Workspace = {
      id: 'ws-2', name: 'WS 2', folderPath: 'C:\\ws2', tabOrder: [],
      shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
    };

    beforeEach(() => {
      store.addWorkspace(ws1);
      store.addWorkspace(ws2);
      store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't4', workspaceId: 'ws-2', name: 'Tab 4', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't5', workspaceId: 'ws-2', name: 'Tab 5', processName: 'cmd', order: 0 });
    });

    it('should restore last active tab when switching back to a workspace', () => {
      store.setActiveWorkspace('ws-1');
      store.setActiveTerminal('t2');

      store.setActiveWorkspace('ws-2');
      store.setActiveWorkspace('ws-1');

      expect(store.getState().activeTerminalId).toBe('t2');
    });

    it('should restore last active tab in the second workspace too', () => {
      store.setActiveWorkspace('ws-2');
      store.setActiveTerminal('t5');

      store.setActiveWorkspace('ws-1');
      store.setActiveWorkspace('ws-2');

      expect(store.getState().activeTerminalId).toBe('t5');
    });

    it('should track active tab changes within a workspace', () => {
      store.setActiveWorkspace('ws-1');
      store.setActiveTerminal('t2');
      store.setActiveTerminal('t3');

      store.setActiveWorkspace('ws-2');
      store.setActiveWorkspace('ws-1');

      expect(store.getState().activeTerminalId).toBe('t3');
    });

    it('should fall back to first tab if remembered tab was removed', () => {
      store.setActiveWorkspace('ws-1');
      store.setActiveTerminal('t2');

      store.setActiveWorkspace('ws-2');
      store.removeTerminal('t2');
      store.setActiveWorkspace('ws-1');

      expect(store.getState().activeTerminalId).toBe('t1');
    });

    it('should remember newly added terminal as active when switching back', () => {
      store.setActiveWorkspace('ws-1');
      store.addTerminal({ id: 't6', workspaceId: 'ws-1', name: 'Tab 6', processName: 'cmd', order: 0 });
      // addTerminal sets the new terminal as active

      store.setActiveWorkspace('ws-2');
      store.setActiveWorkspace('ws-1');

      expect(store.getState().activeTerminalId).toBe('t6');
    });
  });

  describe('OSC title support', () => {
    // Bug: terminal tab name didn't update when programs (like Claude Code) set
    // the title via OSC escape sequences, even though this works in Windows Terminal.

    beforeEach(() => {
      store.addWorkspace({
        id: 'ws-1', name: 'Test', folderPath: 'C:\\', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
      });
      store.addTerminal({
        id: 'term-1', workspaceId: 'ws-1', name: '', processName: 'powershell', order: 0,
      });
    });

    it('should store oscTitle when updated', () => {
      store.updateTerminal('term-1', { oscTitle: 'Claude Code' });

      const terminal = store.getState().terminals.find(t => t.id === 'term-1');
      expect(terminal).toBeDefined();
      expect(terminal!.oscTitle).toBe('Claude Code');
    });

    it('should default oscTitle to undefined when not set', () => {
      const terminal = store.getState().terminals.find(t => t.id === 'term-1');
      expect(terminal).toBeDefined();
      expect(terminal!.oscTitle).toBeUndefined();
    });

    it('should clear oscTitle alongside process change update', () => {
      store.updateTerminal('term-1', { oscTitle: 'Claude Code' });
      // Simulates what terminal-service.ts does on process-changed event
      store.updateTerminal('term-1', { processName: 'powershell', oscTitle: '' });

      const terminal = store.getState().terminals.find(t => t.id === 'term-1');
      expect(terminal).toBeDefined();
      expect(terminal!.oscTitle).toBe('');
    });

    it('should preserve oscTitle when processName does not change', () => {
      store.updateTerminal('term-1', { oscTitle: 'Claude Code' });
      // Unrelated update should not clear oscTitle
      store.updateTerminal('term-1', { name: '' });

      const terminal = store.getState().terminals.find(t => t.id === 'term-1');
      expect(terminal).toBeDefined();
      expect(terminal!.oscTitle).toBe('Claude Code');
    });

    it('should not affect other terminals when setting oscTitle', () => {
      store.addTerminal({ id: 'term-2', workspaceId: 'ws-1', name: '', processName: 'cmd', order: 0 });
      store.updateTerminal('term-1', { oscTitle: 'Claude Code' });

      const term2 = store.getState().terminals.find(t => t.id === 'term-2');
      expect(term2).toBeDefined();
      expect(term2!.oscTitle).toBeUndefined();
    });
  });

  describe('subscription', () => {
    it('should notify listeners on state change', () => {
      let notified = false;
      const unsubscribe = store.subscribe(() => {
        notified = true;
      });

      store.addWorkspace({
        id: 'ws-1',
        name: 'Test',
        folderPath: 'C:\\',
        tabOrder: [],
        shellType: { type: 'windows' },
      });

      expect(notified).toBe(true);
      unsubscribe();
    });

    it('should not notify after unsubscribe', () => {
      let count = 0;
      const unsubscribe = store.subscribe(() => {
        count++;
      });

      store.addWorkspace({
        id: 'ws-1',
        name: 'Test',
        folderPath: 'C:\\',
        tabOrder: [],
        shellType: { type: 'windows' },
      });

      expect(count).toBe(1);

      unsubscribe();

      store.addWorkspace({
        id: 'ws-2',
        name: 'Test 2',
        folderPath: 'D:\\',
        tabOrder: [],
        shellType: { type: 'wsl' },
      });

      expect(count).toBe(1); // Should still be 1
    });
  });

  describe('split view operations', () => {
    const ws1: Workspace = {
      id: 'ws-1', name: 'WS 1', folderPath: 'C:\\ws1', tabOrder: [],
      shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
    };

    beforeEach(() => {
      store.addWorkspace(ws1);
      store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });
    });

    it('should create a split view', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');

      const split = store.getSplitView('ws-1');
      expect(split).not.toBeNull();
      expect(split!.leftTerminalId).toBe('t1');
      expect(split!.rightTerminalId).toBe('t2');
      expect(split!.direction).toBe('horizontal');
      expect(split!.ratio).toBe(0.5);
    });

    it('should create a vertical split view', () => {
      store.setSplitView('ws-1', 't1', 't2', 'vertical', 0.7);

      const split = store.getSplitView('ws-1');
      expect(split!.direction).toBe('vertical');
      expect(split!.ratio).toBe(0.7);
    });

    it('should clear a split view', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.clearSplitView('ws-1');

      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should return null for workspace without split', () => {
      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should update split ratio', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.updateSplitRatio('ws-1', 0.3);

      expect(store.getSplitView('ws-1')!.ratio).toBe(0.3);
    });

    it('should not update ratio for nonexistent split', () => {
      store.updateSplitRatio('ws-1', 0.3);
      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should auto-clear split when removing a split terminal', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.removeTerminal('t1');

      expect(store.getSplitView('ws-1')).toBeNull();
      // Remaining terminal should be active
      expect(store.getState().activeTerminalId).toBe('t2');
    });

    it('should auto-clear split when removing the other split terminal', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.removeTerminal('t2');

      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t1');
    });

    it('should not affect split when removing a non-split terminal', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.removeTerminal('t3');

      expect(store.getSplitView('ws-1')).not.toBeNull();
    });

    it('should auto-clear split when moving a split terminal to another workspace', () => {
      store.addWorkspace({
        id: 'ws-2', name: 'WS 2', folderPath: 'C:\\ws2', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
      });
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.moveTerminalToWorkspace('t1', 'ws-2');

      expect(store.getSplitView('ws-1')).toBeNull();
    });

    it('should clean up split when removing workspace', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.removeWorkspace('ws-1');

      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getState().splitViews).toEqual({});
    });

    it('should support independent splits per workspace', () => {
      store.addWorkspace({
        id: 'ws-2', name: 'WS 2', folderPath: 'C:\\ws2', tabOrder: [],
        shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
      });
      store.addTerminal({ id: 't4', workspaceId: 'ws-2', name: 'Tab 4', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't5', workspaceId: 'ws-2', name: 'Tab 5', processName: 'cmd', order: 0 });

      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setSplitView('ws-2', 't4', 't5', 'vertical');

      expect(store.getSplitView('ws-1')!.direction).toBe('horizontal');
      expect(store.getSplitView('ws-2')!.direction).toBe('vertical');

      store.clearSplitView('ws-1');
      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getSplitView('ws-2')).not.toBeNull();
    });

    it('should clear splitViews on reset', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.reset();

      expect(store.getState().splitViews).toEqual({});
    });

    it('should auto-clear split when navigating to a terminal outside the split', () => {
      // Bug: clicking a tab not in the split left the split active,
      // so the clicked tab was never displayed
      store.setActiveWorkspace('ws-1');
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      // Navigate to t3 which is NOT in the split
      store.setActiveTerminal('t3');

      expect(store.getSplitView('ws-1')).toBeNull();
      expect(store.getState().activeTerminalId).toBe('t3');
    });

    it('should preserve split when navigating within the split', () => {
      store.setActiveWorkspace('ws-1');
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t1');

      // Navigate to t2 which IS in the split (e.g. Alt+\ focus other pane)
      store.setActiveTerminal('t2');

      expect(store.getSplitView('ws-1')).not.toBeNull();
      expect(store.getState().activeTerminalId).toBe('t2');
    });

    it('should preserve split when navigating to leftTerminalId', () => {
      store.setActiveWorkspace('ws-1');
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      store.setActiveTerminal('t2');

      store.setActiveTerminal('t1');

      expect(store.getSplitView('ws-1')).not.toBeNull();
      expect(store.getState().activeTerminalId).toBe('t1');
    });

    it('should not clear split when setting active terminal to null', () => {
      store.setActiveWorkspace('ws-1');
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');

      store.setActiveTerminal(null);

      expect(store.getSplitView('ws-1')).not.toBeNull();
    });

    it('should not throw when no active workspace and navigating outside split', () => {
      store.setSplitView('ws-1', 't1', 't2', 'horizontal');
      // No active workspace set — setActiveTerminal should not crash
      store.setActiveTerminal('t3');

      expect(store.getState().activeTerminalId).toBe('t3');
    });
  });
});
