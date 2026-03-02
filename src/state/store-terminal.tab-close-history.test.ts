// @vitest-environment jsdom
import { describe, it, expect, beforeEach, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

import { Store } from './store';
import type { Terminal } from './store';

function makeTerminal(id: string, workspaceId: string, order = 0): Terminal {
  return {
    id,
    workspaceId,
    name: id,
    processName: 'bash',
    order,
  } as Terminal;
}

describe('removeTerminal: history-based tab close', () => {
  let store: Store;
  const wsId = 'ws-1';

  beforeEach(() => {
    store = new Store();
    store.setState({ activeWorkspaceId: wsId });
  });

  it('activates the previously-used tab when closing the active tab', () => {
    // Setup: 3 tabs in workspace
    const tA = makeTerminal('t-a', wsId, 0);
    const tB = makeTerminal('t-b', wsId, 1);
    const tC = makeTerminal('t-c', wsId, 2);

    // Simulate tab history: A → B → C
    store.addTerminal(tA);
    store.setActiveTerminal('t-a');
    store.addTerminal(tB);
    store.setActiveTerminal('t-b');
    store.addTerminal(tC);
    store.setActiveTerminal('t-c');

    // Close C (active) — should go back to B (previous), not A (positional first)
    store.removeTerminal('t-c');

    expect(store.getState().activeTerminalId).toBe('t-b');
  });

  it('falls back to positional neighbor when previous tab was already closed', () => {
    const tA = makeTerminal('t-a', wsId, 0);
    const tB = makeTerminal('t-b', wsId, 1);
    const tC = makeTerminal('t-c', wsId, 2);

    store.addTerminal(tA);
    store.setActiveTerminal('t-a');
    store.addTerminal(tB);
    store.setActiveTerminal('t-b');
    store.addTerminal(tC);
    store.setActiveTerminal('t-c');

    // Close B first (previous), then close C
    store.removeTerminal('t-b');
    store.setActiveTerminal('t-c'); // re-activate C after B removal
    store.removeTerminal('t-c');

    // B is gone, so fall back to positional (A)
    expect(store.getState().activeTerminalId).toBe('t-a');
  });

  it('falls back to positional neighbor when no history exists', () => {
    const tA = makeTerminal('t-a', wsId, 0);
    const tB = makeTerminal('t-b', wsId, 1);

    store.addTerminal(tA);
    store.addTerminal(tB);
    store.setActiveTerminal('t-b');

    // No previous terminal tracked for this workspace (B was the first activation)
    // but A was added as background, so setLastActiveTerminal was called for B only
    store.removeTerminal('t-b');

    expect(store.getState().activeTerminalId).toBe('t-a');
  });

  it('does not change active tab when closing a non-active tab', () => {
    const tA = makeTerminal('t-a', wsId, 0);
    const tB = makeTerminal('t-b', wsId, 1);
    const tC = makeTerminal('t-c', wsId, 2);

    store.addTerminal(tA);
    store.addTerminal(tB);
    store.addTerminal(tC);
    store.setActiveTerminal('t-b');

    // Close C (not active) — B should remain active
    store.removeTerminal('t-c');

    expect(store.getState().activeTerminalId).toBe('t-b');
  });

  it('activates null when closing the last tab in workspace', () => {
    const tA = makeTerminal('t-a', wsId, 0);

    store.addTerminal(tA);
    store.setActiveTerminal('t-a');

    store.removeTerminal('t-a');

    expect(store.getState().activeTerminalId).toBeNull();
  });
});
