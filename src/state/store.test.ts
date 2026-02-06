import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace, Terminal } from './store';

describe('Store', () => {
  beforeEach(() => {
    // Reset store state before each test
    store.setState({
      workspaces: [],
      terminals: [],
      activeWorkspaceId: null,
      activeTerminalId: null,
    });
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
});
