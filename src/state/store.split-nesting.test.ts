import { describe, it, expect, beforeEach, vi } from 'vitest';
import { store, Workspace } from './store';
import { terminalIds, countLeaves } from './split-types';

// Bug #493: Triggering a split on an already-split view replaces the existing
// split instead of nesting. The root cause is that createSplitTerminal() calls
// addTerminal() which clears the layout tree (because the new terminal is
// outside the tree), and then splitTerminalAt() sees no tree and creates a
// fresh 2-pane split.

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

const ws: Workspace = {
  id: 'ws-1', name: 'WS', folderPath: 'C:\\ws', tabOrder: [],
  shellType: { type: 'windows' }, worktreeMode: false, aiToolMode: 'none',
};

function addTerminals(ids: string[]) {
  for (let i = 0; i < ids.length; i++) {
    store.addTerminal({
      id: ids[i], workspaceId: 'ws-1', name: `Tab ${i + 1}`,
      processName: 'cmd', order: i,
    }, { background: true });
  }
}

/**
 * Simulates the exact sequence of store operations that App.createSplitTerminal()
 * performs: save active ID → addTerminal (non-background) → splitTerminalAt.
 *
 * This is the code path from src/components/App.ts:604-618:
 *   const currentActiveId = state.activeTerminalId;
 *   const newId = await this.createNewTerminal();  // calls addTerminal
 *   store.splitTerminalAt(wsId, currentActiveId, newId, direction);
 */
function simulateCreateSplitTerminal(
  newTerminalId: string,
  direction: 'horizontal' | 'vertical',
) {
  const state = store.getState();
  const currentActiveId = state.activeTerminalId!;
  const wsId = state.activeWorkspaceId!;

  // createNewTerminal() internally calls addTerminal (non-background)
  store.addTerminal({
    id: newTerminalId, workspaceId: wsId, name: `New Tab`,
    processName: 'cmd', order: 0,
  });

  // Then splitTerminalAt is called with the saved currentActiveId
  store.splitTerminalAt(wsId, currentActiveId, newTerminalId, direction);
}

describe('Bug #493: split replaces existing split instead of nesting', () => {
  beforeEach(() => {
    store.reset();
    store.addWorkspace(ws);
  });

  it('should nest a new split inside an existing 2-pane split', () => {
    // Setup: [t1 | t2] horizontal split, t1 is focused
    addTerminals(['t1', 't2']);
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    // Verify initial state: 2-pane split
    expect(countLeaves(store.getLayoutTree('ws-1')!)).toBe(2);

    // Action: trigger split on t1 (simulates Ctrl+\)
    simulateCreateSplitTerminal('t3', 'horizontal');

    // Expected: [t1 | t3] nested inside original → [[t1|t3] | t2] = 3 panes
    const tree = store.getLayoutTree('ws-1');
    expect(tree).not.toBeNull();
    expect(countLeaves(tree!)).toBe(3);
    expect(terminalIds(tree!)).toContain('t1');
    expect(terminalIds(tree!)).toContain('t2');
    expect(terminalIds(tree!)).toContain('t3');
  });

  it('should nest when splitting the second pane of an existing split', () => {
    // Setup: [t1 | t2] horizontal split, t2 is focused
    addTerminals(['t1', 't2']);
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t2');

    // Action: trigger split on t2
    simulateCreateSplitTerminal('t3', 'vertical');

    // Expected: [t1 | [t2 / t3]] = 3 panes
    const tree = store.getLayoutTree('ws-1');
    expect(tree).not.toBeNull();
    expect(countLeaves(tree!)).toBe(3);
    expect(terminalIds(tree!)).toContain('t1');
    expect(terminalIds(tree!)).toContain('t2');
    expect(terminalIds(tree!)).toContain('t3');
  });

  it('should preserve existing 3-pane layout when adding a 4th pane via split', () => {
    // Setup: [t1 | [t2 / t3]] — 3 panes
    addTerminals(['t1', 't2', 't3']);
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
    store.splitTerminalAt('ws-1', 't2', 't3', 'vertical');
    store.setActiveTerminal('t3');

    // Verify: 3 panes
    expect(countLeaves(store.getLayoutTree('ws-1')!)).toBe(3);

    // Action: trigger split on t3
    simulateCreateSplitTerminal('t4', 'horizontal');

    // Expected: [t1 | [t2 / [t3|t4]]] = 4 panes, all previous panes preserved
    const tree = store.getLayoutTree('ws-1');
    expect(tree).not.toBeNull();
    expect(countLeaves(tree!)).toBe(4);
    expect(terminalIds(tree!)).toContain('t1');
    expect(terminalIds(tree!)).toContain('t2');
    expect(terminalIds(tree!)).toContain('t3');
    expect(terminalIds(tree!)).toContain('t4');
  });

  it('should not replace existing split with a flat 2-pane split', () => {
    // Bug #493: The actual broken behavior — after the split, only 2 panes remain
    // instead of 3, because the tree was cleared and recreated as flat.
    addTerminals(['t1', 't2']);
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    simulateCreateSplitTerminal('t3', 'horizontal');

    const tree = store.getLayoutTree('ws-1');
    // The buggy behavior creates a flat [t1|t3] with only 2 panes, losing t2.
    // The correct behavior should produce 3 panes with all terminals present.
    expect(tree).not.toBeNull();
    const leaves = countLeaves(tree!);
    const ids = terminalIds(tree!);

    // t2 must still be in the tree — if it's missing, the old split was replaced
    expect(ids).toContain('t2');
    expect(leaves).toBeGreaterThanOrEqual(3);
  });

  it('should not lose panes when rapidly splitting multiple times', () => {
    // Simulate 3 rapid splits starting from a single terminal
    addTerminals(['t1']);
    store.setActiveWorkspace('ws-1');
    store.setActiveTerminal('t1');

    // First split: t1 → [t1|t2]
    simulateCreateSplitTerminal('t2', 'horizontal');
    store.setActiveTerminal('t1');

    // Second split: should produce [[t1|t3]|t2]
    simulateCreateSplitTerminal('t3', 'vertical');
    store.setActiveTerminal('t1');

    // Third split: should produce [[[t1|t4]|t3]|t2] or similar nesting
    simulateCreateSplitTerminal('t4', 'horizontal');

    const tree = store.getLayoutTree('ws-1');
    expect(tree).not.toBeNull();
    const ids = terminalIds(tree!);

    // All 4 terminals must be present
    expect(ids).toContain('t1');
    expect(ids).toContain('t2');
    expect(ids).toContain('t3');
    expect(ids).toContain('t4');
    expect(countLeaves(tree!)).toBe(4);
  });
});
