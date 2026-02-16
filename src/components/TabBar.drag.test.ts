// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
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
import { workspaceService } from '../services/workspace-service';

// Make store notifications synchronous in jsdom (avoid requestAnimationFrame batching)
const origRAF = globalThis.requestAnimationFrame;

/**
 * Helper: create a mock DragEvent with a spyable dataTransfer and preventDefault.
 */
function createDragEvent(type: string, data?: Record<string, string>): DragEvent {
  const preventDefaultSpy = vi.fn();
  const storedData: Record<string, string> = { ...data };
  const types = Object.keys(storedData);

  const dataTransfer = {
    effectAllowed: 'uninitialized' as string,
    dropEffect: 'none' as string,
    types,
    setData(format: string, value: string) {
      storedData[format] = value;
      types.push(format);
    },
    getData(format: string) {
      return storedData[format] ?? '';
    },
    setDragImage: vi.fn(),
  };

  const event = new Event(type, { bubbles: true, cancelable: true }) as DragEvent;
  Object.defineProperty(event, 'dataTransfer', { value: dataTransfer, writable: false });
  Object.defineProperty(event, 'preventDefault', { value: preventDefaultSpy, writable: false });

  return event;
}

describe('TabBar drag-and-drop reorder', () => {
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
      claudeCodeMode: false,
    });

    store.setActiveWorkspace('ws-1');

    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 0 });
    store.addTerminal({ id: 't3', workspaceId: 'ws-1', name: 'Tab 3', processName: 'cmd', order: 0 });

    tabBar = new TabBar();
    mountPoint = document.createElement('div');
    document.body.appendChild(mountPoint);
    tabBar.mount(mountPoint);
  });

  afterEach(() => {
    document.body.textContent = '';
    globalThis.requestAnimationFrame = origRAF;
    vi.restoreAllMocks();
  });

  function getTabBarContainer(): HTMLElement {
    return mountPoint.querySelector('.tab-bar')!;
  }

  function getTabsContainer(): HTMLElement {
    // The tabsContainer is the first child div of .tab-bar (flex wrapper for tabs)
    return getTabBarContainer().firstElementChild as HTMLElement;
  }

  function getTabElements(): HTMLElement[] {
    return Array.from(mountPoint.querySelectorAll('.tab'));
  }

  // -- Bug: block cursor appears when dragging over tab bar gap --
  // When dragging a tab over the tab-bar container area (empty space not covered
  // by a .tab element), no dragover handler calls preventDefault(), so the browser
  // shows a forbidden/block cursor instead of the move cursor.

  it('should render all three tabs', () => {
    const tabs = getTabElements();
    expect(tabs.length).toBe(3);
  });

  it('tab bar container should call preventDefault on dragover for tab drags', () => {
    // Bug: the tab-bar container has no dragover handler, so dragging over the
    // empty area (gaps, right side of tab bar) shows the block cursor.
    const tabBarEl = getTabBarContainer();
    const dragOverEvent = createDragEvent('dragover', { 'text/plain': 't1' });

    tabBarEl.dispatchEvent(dragOverEvent);

    // Must call preventDefault to allow drops (shows move cursor, not block)
    expect(dragOverEvent.preventDefault).toHaveBeenCalled();
  });

  it('tabs container should call preventDefault on dragover for tab drags', () => {
    // Bug: the flex-wrapper div that holds tabs has no dragover handler,
    // so dragging over the empty space between/after tabs shows block cursor.
    const tabsContainer = getTabsContainer();
    const dragOverEvent = createDragEvent('dragover', { 'text/plain': 't1' });

    tabsContainer.dispatchEvent(dragOverEvent);

    expect(dragOverEvent.preventDefault).toHaveBeenCalled();
  });

  it('tab bar container should set dropEffect to move on dragover', () => {
    const tabBarEl = getTabBarContainer();
    const dragOverEvent = createDragEvent('dragover', { 'text/plain': 't1' });

    tabBarEl.dispatchEvent(dragOverEvent);

    expect(dragOverEvent.dataTransfer!.dropEffect).toBe('move');
  });

  it('tabs container should set dropEffect to move on dragover', () => {
    const tabsContainer = getTabsContainer();
    const dragOverEvent = createDragEvent('dragover', { 'text/plain': 't1' });

    tabsContainer.dispatchEvent(dragOverEvent);

    expect(dragOverEvent.dataTransfer!.dropEffect).toBe('move');
  });

  it('should NOT call preventDefault for non-tab drags on the container', () => {
    // Drags that don't include text/plain (e.g., workspace reorders) should
    // not be intercepted by the tab bar.
    const tabBarEl = getTabBarContainer();
    const dragOverEvent = createDragEvent('dragover', { 'application/x-workspace-id': 'ws-99' });

    tabBarEl.dispatchEvent(dragOverEvent);

    expect(dragOverEvent.preventDefault).not.toHaveBeenCalled();
  });

  it('individual tabs should still call preventDefault on dragover', () => {
    // Sanity check: existing per-tab dragover handlers should work.
    const tabs = getTabElements();
    expect(tabs.length).toBeGreaterThan(0);

    const dragOverEvent = createDragEvent('dragover', { 'text/plain': 't1' });
    tabs[1].dispatchEvent(dragOverEvent);

    expect(dragOverEvent.preventDefault).toHaveBeenCalled();
    expect(dragOverEvent.dataTransfer!.dropEffect).toBe('move');
  });

  it('should complete a drag-reorder when dropping on the tab bar container', () => {
    // Bug: dropping on the tab bar container (not directly on a tab) does nothing
    // because there is no drop handler on the container.
    const tabs = getTabElements();
    const tabBarEl = getTabBarContainer();

    // Simulate: drag t1, drop on the tab bar container
    const dragStartEvent = createDragEvent('dragstart');
    tabs[0].dispatchEvent(dragStartEvent);

    const dropEvent = createDragEvent('drop', { 'text/plain': 't1' });
    tabBarEl.dispatchEvent(dropEvent);

    // The drop on the container should at least not produce an error.
    // (The reorder might need a target tab, but the container should handle gracefully.)
    expect(dropEvent.preventDefault).toHaveBeenCalled();
  });

  it('dragging tab over another tab should add drag-over class', () => {
    const tabs = getTabElements();

    // Start drag on first tab
    const dragStartEvent = createDragEvent('dragstart');
    tabs[0].dispatchEvent(dragStartEvent);

    // Drag over the second tab
    const dragOverEvent = createDragEvent('dragover', { 'text/plain': 't1' });
    tabs[1].dispatchEvent(dragOverEvent);

    expect(tabs[1].classList.contains('drag-over')).toBe(true);
  });

  it('dropping tab on another tab should trigger reorder', async () => {
    const tabs = getTabElements();

    // Start drag on first tab (t1)
    const dragStartEvent = createDragEvent('dragstart');
    tabs[0].dispatchEvent(dragStartEvent);

    // Drop on second tab (t2)
    const dropEvent = createDragEvent('drop', { 'text/plain': 't1' });
    tabs[1].dispatchEvent(dropEvent);

    // workspaceService.reorderTabs should have been called with new order
    // Original order: t1, t2, t3
    // After dragging t1 to t2's position: t2, t1, t3
    await vi.waitFor(() => {
      expect(workspaceService.reorderTabs).toHaveBeenCalledWith(
        'ws-1',
        expect.arrayContaining(['t1', 't2', 't3'])
      );
    });
  });
});
