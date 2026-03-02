import { describe, it, expect, beforeEach } from 'vitest';
import { store } from './store';

function addWorkspace(id: string) {
  store.addWorkspace({
    id,
    name: `Workspace ${id}`,
    folderPath: '/tmp',
    tabOrder: [],
    shellType: { type: 'windows' },
    worktreeMode: false,
    aiToolMode: 'none',
  });
}

function addTerminal(id: string, wsId: string) {
  store.addTerminal({
    id,
    workspaceId: wsId,
    name: `Terminal ${id}`,
    processName: 'bash',
    order: 0,
  });
}

describe('Store: terminal access history', () => {
  beforeEach(() => {
    store.reset();
  });

  it('should track access history when setActiveTerminal is called', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');
    addTerminal('t2', 'ws-1');
    addTerminal('t3', 'ws-1');

    store.setActiveTerminal('t2');
    store.setActiveTerminal('t3');
    store.setActiveTerminal('t1');

    const history = store.getAccessHistory('ws-1');
    // MRU order: t1 (most recent), t3, t2, ...
    expect(history[0]).toBe('t1');
    expect(history[1]).toBe('t3');
    expect(history[2]).toBe('t2');
  });

  it('should not duplicate entries in access history', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');
    addTerminal('t2', 'ws-1');

    store.setActiveTerminal('t1');
    store.setActiveTerminal('t2');
    store.setActiveTerminal('t1');

    const history = store.getAccessHistory('ws-1');
    // t1 should only appear once (at position 0)
    expect(history.filter(id => id === 't1')).toHaveLength(1);
    expect(history[0]).toBe('t1');
    expect(history[1]).toBe('t2');
  });

  it('should return empty array for workspace with no history', () => {
    const history = store.getAccessHistory('nonexistent');
    expect(history).toEqual([]);
  });

  it('should track history when terminal is added in foreground', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');

    const history = store.getAccessHistory('ws-1');
    expect(history).toContain('t1');
  });

  it('should remove terminal from history when removed', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');
    addTerminal('t2', 'ws-1');

    store.setActiveTerminal('t1');
    store.setActiveTerminal('t2');

    store.removeTerminal('t1');

    const history = store.getAccessHistory('ws-1');
    expect(history).not.toContain('t1');
    expect(history).toContain('t2');
  });

  it('should cap history at 50 entries per workspace', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');

    // Add 60 terminals and activate each
    for (let i = 0; i < 60; i++) {
      const id = `t${i}`;
      store.addTerminal({
        id,
        workspaceId: 'ws-1',
        name: `Terminal ${i}`,
        processName: 'bash',
        order: 0,
      }, { background: true });
    }

    for (let i = 0; i < 60; i++) {
      store.setActiveTerminal(`t${i}`);
    }

    const history = store.getAccessHistory('ws-1');
    expect(history.length).toBeLessThanOrEqual(50);
  });

  it('should maintain separate history per workspace', () => {
    addWorkspace('ws-1');
    addWorkspace('ws-2');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');
    store.setActiveWorkspace('ws-2');
    addTerminal('t2', 'ws-2');

    const h1 = store.getAccessHistory('ws-1');
    const h2 = store.getAccessHistory('ws-2');

    expect(h1).toContain('t1');
    expect(h1).not.toContain('t2');
    expect(h2).toContain('t2');
    expect(h2).not.toContain('t1');
  });

  it('should clear access history on store reset', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');

    expect(store.getAccessHistory('ws-1')).toContain('t1');

    store.reset();

    expect(store.getAccessHistory('ws-1')).toEqual([]);
  });
});
