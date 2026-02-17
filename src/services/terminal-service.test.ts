import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock @tauri-apps/api modules
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

// Capture listen callbacks so we can simulate events
type ListenCallback = (event: { payload: unknown }) => void;
const listenCallbacks: Map<string, ListenCallback> = new Map();

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((eventName: string, callback: ListenCallback) => {
    listenCallbacks.set(eventName, callback);
    return Promise.resolve(() => {
      listenCallbacks.delete(eventName);
    });
  }),
}));

import { store } from '../state/store';
import { terminalService } from './terminal-service';

describe('TerminalService', () => {
  beforeEach(async () => {
    vi.clearAllMocks();
    listenCallbacks.clear();

    // Set up store with a workspace and terminals
    store.setState({
      workspaces: [
        {
          id: 'ws-1',
          name: 'Workspace',
          folderPath: '',
          tabOrder: ['t1', 't2'],
          shellType: { type: 'windows' },
          worktreeMode: false,
          claudeCodeMode: false,
        },
      ],
      terminals: [
        { id: 't1', workspaceId: 'ws-1', name: 'Terminal 1', shellType: { type: 'windows' }, order: 0 },
        { id: 't2', workspaceId: 'ws-1', name: 'Terminal 2', shellType: { type: 'windows' }, order: 1 },
      ],
      activeWorkspaceId: 'ws-1',
      activeTerminalId: 't1',
    });

    // Initialize the service to register event listeners
    await terminalService.init();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('terminal-closed event handling', () => {
    // Bug A2: When a PTY exits, the daemon sends SessionClosed which the bridge
    // translates to a 'terminal-closed' Tauri event. The service should handle
    // this by removing the terminal from the store.

    it('should remove terminal from store when terminal-closed event fires', () => {
      const closedCallback = listenCallbacks.get('terminal-closed');
      expect(closedCallback).toBeDefined();

      // Verify terminal exists before event
      expect(store.getState().terminals.find(t => t.id === 't1')).toBeDefined();

      // Simulate terminal-closed event from the bridge
      closedCallback!({ payload: { terminal_id: 't1' } });

      // Terminal should be removed from the store
      expect(store.getState().terminals.find(t => t.id === 't1')).toBeUndefined();
    });

    it('should clean up output listener when terminal-closed event fires', () => {
      // Register an output listener
      const outputCallback = vi.fn();
      terminalService.onTerminalOutput('t1', outputCallback);

      // Fire terminal-closed
      const closedCallback = listenCallbacks.get('terminal-closed');
      closedCallback!({ payload: { terminal_id: 't1' } });

      // Now fire terminal-output for the same terminal — callback should NOT fire
      const outputEventCallback = listenCallbacks.get('terminal-output');
      outputEventCallback!({ payload: { terminal_id: 't1' } });

      expect(outputCallback).not.toHaveBeenCalled();
    });

    it('should switch active terminal when the active terminal is closed', () => {
      // Make t1 active
      store.setActiveTerminal('t1');
      expect(store.getState().activeTerminalId).toBe('t1');

      // Close t1 via terminal-closed event
      const closedCallback = listenCallbacks.get('terminal-closed');
      closedCallback!({ payload: { terminal_id: 't1' } });

      // Active terminal should switch to t2 (the remaining terminal in the same workspace)
      expect(store.getState().activeTerminalId).toBe('t2');
    });

    it('should handle terminal-closed for unknown terminal gracefully', () => {
      const closedCallback = listenCallbacks.get('terminal-closed');

      // Should not throw for a terminal that doesn't exist
      expect(() => {
        closedCallback!({ payload: { terminal_id: 'nonexistent' } });
      }).not.toThrow();
    });

    it('should remove terminal from the terminals array', () => {
      const closedCallback = listenCallbacks.get('terminal-closed');
      closedCallback!({ payload: { terminal_id: 't1' } });

      // Only t2 should remain
      const terminalIds = store.getState().terminals.map(t => t.id);
      expect(terminalIds).toEqual(['t2']);
    });
  });

  describe('terminal-output event routing', () => {
    it('should route output events to registered listeners', () => {
      const callback = vi.fn();
      terminalService.onTerminalOutput('t1', callback);

      const outputCallback = listenCallbacks.get('terminal-output');
      outputCallback!({ payload: { terminal_id: 't1' } });

      expect(callback).toHaveBeenCalledTimes(1);
    });

    it('should not route output events for terminals without listeners', () => {
      const callback = vi.fn();
      terminalService.onTerminalOutput('t1', callback);

      const outputCallback = listenCallbacks.get('terminal-output');
      // Fire for t2, which has no listener
      outputCallback!({ payload: { terminal_id: 't2' } });

      expect(callback).not.toHaveBeenCalled();
    });
  });
});
