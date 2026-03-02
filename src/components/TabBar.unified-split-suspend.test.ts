// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';
import { terminalSettingsStore } from '../state/terminal-settings-store';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('../services/terminal-service', () => ({
  terminalService: {
    createTerminal: vi.fn(),
    closeTerminal: vi.fn(),
    writeToTerminal: vi.fn(),
    renameTerminal: vi.fn(),
  },
}));

vi.mock('../services/workspace-service', () => ({
  workspaceService: {
    reorderTabs: vi.fn(() => Promise.resolve()),
  },
}));

import { TabBar } from './TabBar';

// Make store notifications synchronous in jsdom (avoid requestAnimationFrame batching)
const origRAF = globalThis.requestAnimationFrame;

// Bug #509: In split view with unified mode, navigating to a tab outside
// the split pair causes the unified tab to disappear. Individual tabs for
// each split member appear instead. The split layout is correctly suspended
// in the store, but the TabBar's buildRenderItems() returns early at line 313
// when treeIdSet.size === 0 (the active tree is cleared on suspension),
// before ever reaching the suspended tree check at lines 319-322.

describe('BUG #509: unified split tab disappears when switching tabs', () => {
  let tabBar: TabBar;
  let mountPoint: HTMLElement;

  beforeEach(() => {
    // Make requestAnimationFrame synchronous for test predictability
    globalThis.requestAnimationFrame = (cb: FrameRequestCallback) => { cb(0); return 0; };

    store.reset();

    store.addWorkspace({
      id: 'ws-1',
      name: 'Test Workspace',
      folderPath: 'C:\\test',
      tabOrder: [],
      shellType: { type: 'windows' },
      worktreeMode: false,
      aiToolMode: 'none',
    });

    store.setActiveWorkspace('ws-1');

    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });

    // Enable unified split tab mode
    terminalSettingsStore.setSplitTabMode('unified');

    // Create a split between t1 and t2
    store.splitTerminalAt('ws-1', 't1', 't2', 'horizontal');
    store.setActiveTerminal('t1');

    tabBar = new TabBar();
    mountPoint = document.createElement('div');
    document.body.appendChild(mountPoint);
    tabBar.mount(mountPoint);
  });

  afterEach(() => {
    document.body.textContent = '';
    globalThis.requestAnimationFrame = origRAF;
    terminalSettingsStore.setSplitTabMode('individual');
    vi.restoreAllMocks();
  });

  function getTabElements(): HTMLElement[] {
    return Array.from(mountPoint.querySelectorAll('.tab'));
  }

  function getUnifiedTab(): HTMLElement | null {
    return mountPoint.querySelector('.unified-split-tab');
  }

  function getIndividualTabIds(): string[] {
    return getTabElements()
      .filter(el => !el.classList.contains('unified-split-tab'))
      .map(el => el.dataset.terminalId!)
      .filter(Boolean);
  }

  it('should render unified tab when split is active', () => {
    // Sanity check: unified tab exists when split is active
    const unified = getUnifiedTab();
    expect(unified).not.toBeNull();

    // Should have 2 tabs total: 1 unified (t1+t2) + 1 individual (t3)
    const tabs = getTabElements();
    expect(tabs.length).toBe(2);
  });

  it('should preserve unified tab when navigating away from split', () => {
    // Bug #509: clicking t3 suspends the split, but the unified tab should
    // remain visible so the user can click it to restore the split.

    // Verify unified tab exists before navigating away
    expect(getUnifiedTab()).not.toBeNull();

    // Navigate to t3 (outside the split pair) — split suspends
    store.setActiveTerminal('t3');

    // The unified tab should STILL be rendered (showing suspended split)
    const unified = getUnifiedTab();
    expect(unified).not.toBeNull();

    // Should still be 2 tabs: unified (t1+t2 suspended) + t3
    const tabs = getTabElements();
    expect(tabs.length).toBe(2);
  });

  it('should not show individual tabs for split members when split is suspended', () => {
    // Bug #509: after navigating away, t1 and t2 should NOT appear as
    // separate individual tabs — they should remain grouped in the unified tab.

    store.setActiveTerminal('t3');

    // Individual tabs should only contain t3
    const individualIds = getIndividualTabIds();
    expect(individualIds).not.toContain('t1');
    expect(individualIds).not.toContain('t2');
    expect(individualIds).toContain('t3');
  });

  it('should restore split when clicking unified tab after navigating away', () => {
    // Navigate away
    store.setActiveTerminal('t3');

    // Verify split is suspended (store-level, should work per existing tests)
    expect(store.getLayoutTree('ws-1')).toBeNull();
    expect(store.getSuspendedLayoutTree('ws-1')).not.toBeNull();

    // Click unified tab — should restore the split
    const unified = getUnifiedTab();
    expect(unified).not.toBeNull();
    unified!.click();

    // Split should be restored
    expect(store.getLayoutTree('ws-1')).not.toBeNull();
    expect(store.getSuspendedLayoutTree('ws-1')).toBeUndefined();
  });

  it('should keep unified tab after multiple tab switches', () => {
    // Bug #509: round-trip: split → t3 → t1 (restore) → t3 again
    // The unified tab should survive the full cycle.

    // Navigate away to t3
    store.setActiveTerminal('t3');
    expect(getUnifiedTab()).not.toBeNull();

    // Navigate back to t1 (restores split)
    store.setActiveTerminal('t1');
    expect(getUnifiedTab()).not.toBeNull();

    // Navigate away again to t3
    store.setActiveTerminal('t3');
    expect(getUnifiedTab()).not.toBeNull();

    // Total tabs should be 2 throughout: unified + t3
    expect(getTabElements().length).toBe(2);
  });

  it('unified tab should be inactive when viewing a non-split tab', () => {
    // Navigate away
    store.setActiveTerminal('t3');

    const unified = getUnifiedTab();
    expect(unified).not.toBeNull();
    expect(unified!.classList.contains('active')).toBe(false);
  });
});
