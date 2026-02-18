import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock @tauri-apps/api modules
const mockInvoke = vi.fn(() => Promise.resolve());
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
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
        { id: 't1', workspaceId: 'ws-1', name: 'Terminal 1', processName: 'powershell', order: 0 },
        { id: 't2', workspaceId: 'ws-1', name: 'Terminal 2', processName: 'powershell', order: 1 },
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
    // When a PTY exits, the daemon sends SessionClosed which the bridge
    // translates to a 'terminal-closed' Tauri event. The service should
    // mark the terminal as exited (not remove it) so the tab stays visible.

    it('should mark terminal as exited instead of removing it', () => {
      const closedCallback = listenCallbacks.get('terminal-closed');
      expect(closedCallback).toBeDefined();

      // Verify terminal exists and is not exited before event
      const before = store.getState().terminals.find(t => t.id === 't1');
      expect(before).toBeDefined();
      expect(before!.exited).toBeFalsy();

      // Simulate terminal-closed event from the bridge
      closedCallback!({ payload: { terminal_id: 't1' } });

      // Terminal should still exist but be marked as exited
      const after = store.getState().terminals.find(t => t.id === 't1');
      expect(after).toBeDefined();
      expect(after!.exited).toBe(true);
    });

    it('should call close_terminal invoke to free daemon resources', () => {
      const closedCallback = listenCallbacks.get('terminal-closed');
      closedCallback!({ payload: { terminal_id: 't1' } });

      // close_terminal should be called fire-and-forget to release daemon memory
      expect(mockInvoke).toHaveBeenCalledWith('close_terminal', { terminalId: 't1' });
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

    it('should keep terminal in the terminals array (tab remains visible)', () => {
      const closedCallback = listenCallbacks.get('terminal-closed');
      closedCallback!({ payload: { terminal_id: 't1' } });

      // Both terminals should still be in the array
      const terminalIds = store.getState().terminals.map(t => t.id);
      expect(terminalIds).toContain('t1');
      expect(terminalIds).toContain('t2');
    });

    it('should not change active terminal when a terminal exits', () => {
      store.setActiveTerminal('t1');
      expect(store.getState().activeTerminalId).toBe('t1');

      const closedCallback = listenCallbacks.get('terminal-closed');
      closedCallback!({ payload: { terminal_id: 't1' } });

      // Active terminal should remain t1 (tab stays visible with overlay)
      expect(store.getState().activeTerminalId).toBe('t1');
    });

    it('should handle terminal-closed for unknown terminal gracefully', () => {
      const closedCallback = listenCallbacks.get('terminal-closed');

      // Should not throw for a terminal that doesn't exist
      expect(() => {
        closedCallback!({ payload: { terminal_id: 'nonexistent' } });
      }).not.toThrow();
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
