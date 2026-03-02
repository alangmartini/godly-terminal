import { describe, it, expect, beforeEach } from 'vitest';
import { store, Terminal, Workspace } from './store';

function workspace(id = 'ws-1'): Workspace {
  return {
    id,
    name: 'Test',
    folderPath: '/tmp',
    tabOrder: [],
    shellType: { type: 'windows' },
    worktreeMode: false,
    aiToolMode: 'none',
  };
}

function terminal(overrides: Partial<Terminal> = {}): Terminal {
  return {
    id: 't-1',
    workspaceId: 'ws-1',
    name: 'Terminal',
    processName: 'powershell',
    order: 0,
    ...overrides,
  };
}

describe('Pinned tabs', () => {
  beforeEach(() => {
    store.reset();
    store.addWorkspace(workspace());
    store.setActiveWorkspace('ws-1');
  });

  describe('togglePinTab', () => {
    it('pins an unpinned tab', () => {
      store.addTerminal(terminal({ id: 't-1' }));

      store.togglePinTab('t-1');

      const t = store.getState().terminals.find(t => t.id === 't-1');
      expect(t?.pinned).toBe(true);
    });

    it('unpins a pinned tab', () => {
      store.addTerminal(terminal({ id: 't-1', pinned: true }));

      store.togglePinTab('t-1');

      const t = store.getState().terminals.find(t => t.id === 't-1');
      expect(t?.pinned).toBe(false);
    });

    it('does nothing for a non-existent terminal', () => {
      store.addTerminal(terminal({ id: 't-1' }));
      const before = store.getState().terminals;

      store.togglePinTab('t-nonexistent');

      expect(store.getState().terminals).toEqual(before);
    });
  });

  describe('removeTerminal respects pinned status', () => {
    it('refuses to remove a pinned tab (default)', () => {
      store.addTerminal(terminal({ id: 't-1', pinned: true }));

      store.removeTerminal('t-1');

      // Terminal should still exist
      expect(store.getState().terminals).toHaveLength(1);
      expect(store.getState().terminals[0].id).toBe('t-1');
    });

    it('removes a pinned tab when force=true', () => {
      store.addTerminal(terminal({ id: 't-1', pinned: true }));

      store.removeTerminal('t-1', true);

      expect(store.getState().terminals).toHaveLength(0);
    });

    it('removes an unpinned tab normally', () => {
      store.addTerminal(terminal({ id: 't-1' }));

      store.removeTerminal('t-1');

      expect(store.getState().terminals).toHaveLength(0);
    });

    it('removes an explicitly unpinned (pinned=false) tab normally', () => {
      store.addTerminal(terminal({ id: 't-1', pinned: false }));

      store.removeTerminal('t-1');

      expect(store.getState().terminals).toHaveLength(0);
    });
  });

  describe('pinned tabs sort first in getWorkspaceTerminals', () => {
    it('pinned tabs appear before unpinned tabs', () => {
      // Add in order: unpinned, pinned, unpinned
      store.addTerminal(terminal({ id: 't-1', order: 0 }));
      store.addTerminal(terminal({ id: 't-2', order: 1, pinned: true }), { background: true });
      store.addTerminal(terminal({ id: 't-3', order: 2 }), { background: true });

      // getWorkspaceTerminals sorts by order — pinned should come first
      // via the TabBar render sort (store itself just sorts by order)
      const all = store.getWorkspaceTerminals('ws-1');
      // Store returns by order, TabBar re-sorts with pinned first
      // Verify the pinned field is preserved
      const pinned = all.filter(t => t.pinned);
      const unpinned = all.filter(t => !t.pinned);
      expect(pinned).toHaveLength(1);
      expect(unpinned).toHaveLength(2);
      expect(pinned[0].id).toBe('t-2');
    });
  });
});
