import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from '../state/store';
import { terminalSettingsStore } from '../state/terminal-settings-store';
import { terminalIds, LayoutNode } from '../state/split-types';

// Bug #508: Split tabs default to individual mode — should default to unified.
// When split pane view is active, each terminal appears as a separate tab in
// the tab bar, cluttering the UI. The default splitTabMode should be 'unified'
// so split terminals consolidate into a single tab entry.

/**
 * Replicates TabBar.buildRenderItems() logic to test without DOM dependencies.
 * This is the exact algorithm from TabBar.ts:305-345.
 */
function buildRenderItems(
  terminals: { id: string }[],
  wsId: string,
): { id: string; terminals: { id: string }[] }[] {
  const UNIFIED_KEY = '__unified_split__';
  const isUnified = terminalSettingsStore.getSplitTabMode() === 'unified';
  const tree = wsId ? store.getLayoutTree(wsId) : null;
  const treeIdSet = tree ? new Set(terminalIds(tree)) : new Set<string>();

  if (!isUnified || treeIdSet.size === 0) {
    return terminals.map(t => ({ id: t.id, terminals: [t] }));
  }

  const suspended = wsId ? store.getSuspendedLayoutTree(wsId) : undefined;
  const suspendedIdSet = suspended
    ? new Set(terminalIds(suspended.tree))
    : new Set<string>();

  const splitIds = treeIdSet.size > 0 ? treeIdSet : suspendedIdSet;
  if (splitIds.size === 0) {
    return terminals.map(t => ({ id: t.id, terminals: [t] }));
  }

  const items: { id: string; terminals: { id: string }[] }[] = [];
  let unifiedInserted = false;
  const splitTerminals = terminals.filter(t => splitIds.has(t.id));

  for (const t of terminals) {
    if (splitIds.has(t.id)) {
      if (!unifiedInserted) {
        items.push({ id: UNIFIED_KEY, terminals: splitTerminals });
        unifiedInserted = true;
      }
    } else {
      items.push({ id: t.id, terminals: [t] });
    }
  }
  return items;
}

describe('split tab unified default (#508)', () => {
  const ws: Workspace = {
    id: 'ws-1', name: 'WS', folderPath: 'C:\\ws', tabOrder: [],
    shellType: { type: 'windows' }, worktreeMode: false, aiToolMode: 'none',
  };

  beforeEach(() => {
    store.reset();
    // Clear localStorage to simulate fresh install (no persisted settings)
    if (typeof localStorage !== 'undefined') {
      localStorage.removeItem('godly-terminal-settings');
    }
  });

  describe('default splitTabMode should be unified', () => {
    it('fresh install default should be "unified"', () => {
      // Bug #508: Default is 'individual', causing split tabs to show as
      // separate entries in the tab bar. Should be 'unified' by default.
      expect(terminalSettingsStore.getSplitTabMode()).toBe('unified');
    });
  });

  describe('buildRenderItems with default mode should consolidate split tabs', () => {
    beforeEach(() => {
      store.addWorkspace(ws);
      store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
      store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 1 });
      store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 2 });
      store.setActiveWorkspace('ws-1');
      // Create a split between t1 and t2
      store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal', 0.5);
      store.setActiveTerminal('t1');
    });

    it('split terminals should appear as one unified entry, not two individual tabs', () => {
      // Bug #508: With default mode, split terminals t1 and t2 should be
      // consolidated into a single render item, leaving t3 as individual.
      const terminals = store.getWorkspaceTerminals('ws-1');
      const items = buildRenderItems(terminals, 'ws-1');

      // Expected: 2 items — one unified entry for [t1, t2] and one for t3
      // Bug: 3 items — t1, t2, t3 all as individual entries
      expect(items).toHaveLength(2);

      const unifiedItem = items.find(i => i.id === '__unified_split__');
      expect(unifiedItem).toBeDefined();
      expect(unifiedItem!.terminals).toHaveLength(2);
      expect(unifiedItem!.terminals.map(t => t.id)).toContain('t1');
      expect(unifiedItem!.terminals.map(t => t.id)).toContain('t2');
    });

    it('non-split terminals should remain as individual entries', () => {
      const terminals = store.getWorkspaceTerminals('ws-1');
      const items = buildRenderItems(terminals, 'ws-1');

      const t3Item = items.find(i => i.terminals.some(t => t.id === 't3'));
      expect(t3Item).toBeDefined();
      expect(t3Item!.id).toBe('t3');
      expect(t3Item!.terminals).toHaveLength(1);
    });

    it('all terminals in workspace should be accounted for', () => {
      const terminals = store.getWorkspaceTerminals('ws-1');
      const items = buildRenderItems(terminals, 'ws-1');

      // All 3 terminals should be present across the render items
      const allIds = items.flatMap(i => i.terminals.map(t => t.id));
      expect(allIds.sort()).toEqual(['t1', 't2', 't3']);
    });
  });

  // Note: suspended split consolidation is tracked separately as #509 (PR #510).
});
