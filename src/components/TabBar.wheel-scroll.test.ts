// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';

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

describe('Bug #189: Tab bar does not scroll with mouse wheel when tabs overflow', () => {
  let tabBar: TabBar;
  let mountPoint: HTMLElement;

  beforeEach(() => {
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

    tabBar = new TabBar();
    mountPoint = document.createElement('div');
    document.body.appendChild(mountPoint);
    tabBar.mount(mountPoint);
  });

  afterEach(() => {
    document.body.textContent = '';
    vi.restoreAllMocks();
  });

  function addTabs(count: number) {
    for (let i = 0; i < count; i++) {
      store.addTerminal({
        id: `t-${i}`,
        workspaceId: 'ws-1',
        name: `Terminal ${i}`,
        processName: 'cmd',
        order: 0,
      });
    }
  }

  // Bug #189: vertical mouse wheel events on the tab bar should scroll tabs
  // horizontally. Currently no wheel event listener exists, so deltaY is ignored
  // and tabs cannot be scrolled when they overflow.

  it('should translate vertical wheel deltaY into horizontal scrollLeft change', () => {
    addTabs(10);
    const tabBarEl = mountPoint.querySelector('.tab-bar') as HTMLElement;

    // Mock scrollLeft since jsdom has no real layout engine
    let currentScrollLeft = 0;
    Object.defineProperty(tabBarEl, 'scrollLeft', {
      get() { return currentScrollLeft; },
      set(val: number) { currentScrollLeft = val; },
      configurable: true,
    });

    // Dispatch a vertical wheel event (scroll down = positive deltaY)
    const wheelEvent = new WheelEvent('wheel', {
      deltaY: 100,
      bubbles: true,
      cancelable: true,
    });
    tabBarEl.dispatchEvent(wheelEvent);

    // A wheel handler should translate deltaY into scrollLeft increase
    expect(currentScrollLeft).toBeGreaterThan(0);
  });

  it('should scroll tabs left when wheel scrolls up (negative deltaY)', () => {
    addTabs(10);
    const tabBarEl = mountPoint.querySelector('.tab-bar') as HTMLElement;

    // Start scrolled to the middle
    let currentScrollLeft = 200;
    Object.defineProperty(tabBarEl, 'scrollLeft', {
      get() { return currentScrollLeft; },
      set(val: number) { currentScrollLeft = val; },
      configurable: true,
    });

    const wheelEvent = new WheelEvent('wheel', {
      deltaY: -100,
      bubbles: true,
      cancelable: true,
    });
    tabBarEl.dispatchEvent(wheelEvent);

    // scrollLeft should decrease when scrolling up
    expect(currentScrollLeft).toBeLessThan(200);
  });

  it('should prevent default vertical scroll behavior on wheel events', () => {
    addTabs(10);
    const tabBarEl = mountPoint.querySelector('.tab-bar') as HTMLElement;

    const wheelEvent = new WheelEvent('wheel', {
      deltaY: 100,
      bubbles: true,
      cancelable: true,
    });
    tabBarEl.dispatchEvent(wheelEvent);

    // The handler should call preventDefault() to stop the page from
    // scrolling vertically while the user is scrolling the tab bar
    expect(wheelEvent.defaultPrevented).toBe(true);
  });

  it('should accumulate scroll across multiple wheel events', () => {
    addTabs(10);
    const tabBarEl = mountPoint.querySelector('.tab-bar') as HTMLElement;

    let currentScrollLeft = 0;
    Object.defineProperty(tabBarEl, 'scrollLeft', {
      get() { return currentScrollLeft; },
      set(val: number) { currentScrollLeft = val; },
      configurable: true,
    });

    // Dispatch three wheel events
    for (let i = 0; i < 3; i++) {
      tabBarEl.dispatchEvent(new WheelEvent('wheel', {
        deltaY: 50,
        bubbles: true,
        cancelable: true,
      }));
    }

    // scrollLeft should reflect all three scroll increments
    expect(currentScrollLeft).toBeGreaterThan(50);
  });
});
