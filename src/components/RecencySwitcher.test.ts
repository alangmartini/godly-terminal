import { describe, it, expect, beforeEach } from 'vitest';
import { store } from '../state/store';
import { RecencySwitcher } from './RecencySwitcher';

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

function addTerminal(id: string, wsId: string, name?: string) {
  store.addTerminal({
    id,
    workspaceId: wsId,
    name: name ?? `Terminal ${id}`,
    processName: 'bash',
    order: 0,
  });
}

// RecencySwitcher uses document.createElement, so full DOM tests require
// a browser environment. These tests use JSDOM-like stubs for basic checks.
// Full interaction tests belong in *.browser.test.ts.

describe('RecencySwitcher (unit — buildMruList logic)', () => {
  beforeEach(() => {
    store.reset();
  });

  it('should expose access history in MRU order via store', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');
    addTerminal('t2', 'ws-1');
    addTerminal('t3', 'ws-1');

    store.setActiveTerminal('t1');
    store.setActiveTerminal('t2');
    store.setActiveTerminal('t3');

    const history = store.getAccessHistory('ws-1');
    expect(history).toEqual(['t3', 't2', 't1']);
  });

  it('should order MRU with most recent first after switching back and forth', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');
    addTerminal('t2', 'ws-1');
    addTerminal('t3', 'ws-1');

    store.setActiveTerminal('t1');
    store.setActiveTerminal('t2');
    store.setActiveTerminal('t3');
    store.setActiveTerminal('t1'); // go back to t1

    const history = store.getAccessHistory('ws-1');
    expect(history[0]).toBe('t1');
    expect(history[1]).toBe('t3');
    expect(history[2]).toBe('t2');
  });

  it('should provide all workspace terminals even if not in history', () => {
    addWorkspace('ws-1');
    store.setActiveWorkspace('ws-1');
    addTerminal('t1', 'ws-1');
    // Add t2 in background (no access history entry)
    store.addTerminal({
      id: 't2',
      workspaceId: 'ws-1',
      name: 'Background',
      processName: 'bash',
      order: 0,
    }, { background: true });

    const history = store.getAccessHistory('ws-1');
    const wsTerminals = store.getWorkspaceTerminals('ws-1');

    // t1 is in history, t2 is not
    expect(history).toContain('t1');
    expect(history).not.toContain('t2');
    // But both are workspace terminals
    expect(wsTerminals.map(t => t.id)).toContain('t1');
    expect(wsTerminals.map(t => t.id)).toContain('t2');
  });
});
