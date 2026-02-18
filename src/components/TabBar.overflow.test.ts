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

const origRAF = globalThis.requestAnimationFrame;

// CSS constants from main.css
const TAB_MIN_WIDTH = 120; // .tab { min-width: 120px }
const ADD_BTN_WIDTH = 35;  // .add-tab-btn { width: 35px }

describe('TabBar add-button overlap with many tabs', () => {
  let tabBar: TabBar;
  let mountPoint: HTMLElement;

  beforeEach(() => {
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

  function getTabBarElements() {
    const tabBarEl = mountPoint.querySelector('.tab-bar') as HTMLElement;
    const tabsContainer = tabBarEl.children[0] as HTMLElement;
    const addBtn = tabBarEl.querySelector('.add-tab-btn') as HTMLElement;
    const tabs = Array.from(tabsContainer.querySelectorAll('.tab')) as HTMLElement[];
    return { tabBarEl, tabsContainer, addBtn, tabs };
  }

  // Bug: When many tabs are open, the "+" button overlaps with tab content.
  // Root cause: tabsContainer has inline min-width: 0, which allows the flex
  // item to shrink below its content width. Tabs (each with CSS min-width: 120px)
  // overflow the container, while the "+" button sits at the shrunken container's
  // end — visually appearing in the middle of the overflowing tabs.

  it('add button should not overlap tabs when tab count exceeds viewport width', () => {
    const TAB_COUNT = 8;
    addTabs(TAB_COUNT);

    const { tabsContainer, tabs } = getTabBarElements();
    expect(tabs).toHaveLength(TAB_COUNT);

    const totalTabsMinWidth = TAB_COUNT * TAB_MIN_WIDTH; // 960px
    const VIEWPORT_WIDTH = 800; // Typical narrow viewport

    // Simulate CSS flex layout algorithm:
    // In a flex row with overflow-x: auto, a flex:1 child's width is:
    //   max(available_space_after_siblings, child_min_width)
    //
    // When min-width is 'auto' (default for flex items), the child cannot
    // shrink below its content's minimum size (sum of children min-widths).
    //
    // When min-width is '0', the child CAN shrink to 0, causing content to
    // overflow the child. The next sibling (add button) is positioned after
    // the *shrunken* child, not after the overflowing content.

    const inlineMinWidth = tabsContainer.style.minWidth;
    const minWidthIsZero = inlineMinWidth === '0' || inlineMinWidth === '0px';

    // With min-width: 0, flex allocates: VIEWPORT - ADD_BTN = 765px to tabsContainer
    // Tabs overflow to 960px, but addBtn starts at 765px — overlap!
    //
    // With min-width: auto (default), flex allocates: max(765, 960) = 960px
    // Tab-bar scrolls via overflow-x: auto, addBtn starts at 960px — no overlap.
    const effectiveMinWidth = minWidthIsZero ? 0 : totalTabsMinWidth;
    const tabsContainerWidth = Math.max(VIEWPORT_WIDTH - ADD_BTN_WIDTH, effectiveMinWidth);
    const addBtnLeftEdge = tabsContainerWidth;

    expect(addBtnLeftEdge).toBeGreaterThanOrEqual(totalTabsMinWidth);
  });

  it('add button should not overlap tabs in a maximized window', () => {
    const TAB_COUNT = 12;
    addTabs(TAB_COUNT);

    const { tabsContainer, tabs } = getTabBarElements();
    expect(tabs).toHaveLength(TAB_COUNT);

    const totalTabsMinWidth = TAB_COUNT * TAB_MIN_WIDTH; // 1440px
    const VIEWPORT_WIDTH = 1920; // Full HD maximized

    const inlineMinWidth = tabsContainer.style.minWidth;
    const minWidthIsZero = inlineMinWidth === '0' || inlineMinWidth === '0px';

    const effectiveMinWidth = minWidthIsZero ? 0 : totalTabsMinWidth;
    const tabsContainerWidth = Math.max(VIEWPORT_WIDTH - ADD_BTN_WIDTH, effectiveMinWidth);
    const addBtnLeftEdge = tabsContainerWidth;

    // With 12 tabs at 120px each = 1440px, fits in 1920px viewport.
    // No overlap in either case for this count/viewport combo.
    // This test passes even with the bug — included for completeness.
    expect(addBtnLeftEdge).toBeGreaterThanOrEqual(totalTabsMinWidth);
  });

  it('add button should not overlap tabs in a narrow window with many tabs', () => {
    const TAB_COUNT = 6;
    addTabs(TAB_COUNT);

    const { tabsContainer, tabs } = getTabBarElements();
    expect(tabs).toHaveLength(TAB_COUNT);

    const totalTabsMinWidth = TAB_COUNT * TAB_MIN_WIDTH; // 720px
    const VIEWPORT_WIDTH = 600; // Very narrow window

    const inlineMinWidth = tabsContainer.style.minWidth;
    const minWidthIsZero = inlineMinWidth === '0' || inlineMinWidth === '0px';

    // With min-width: 0: tabsContainer = 600 - 35 = 565px, tabs overflow to 720px
    // Add button at 565px < 720px — OVERLAP
    const effectiveMinWidth = minWidthIsZero ? 0 : totalTabsMinWidth;
    const tabsContainerWidth = Math.max(VIEWPORT_WIDTH - ADD_BTN_WIDTH, effectiveMinWidth);
    const addBtnLeftEdge = tabsContainerWidth;

    expect(addBtnLeftEdge).toBeGreaterThanOrEqual(totalTabsMinWidth);
  });

  it('tabs container should not allow flex shrink below content minimum width', () => {
    addTabs(8);

    const { tabsContainer } = getTabBarElements();

    // The tabsContainer should not have min-width: 0 which allows shrinking
    // below the combined min-width of its tab children. In CSS flex layout,
    // the default min-width: auto prevents this by respecting content size.
    // Explicitly setting min-width: 0 overrides this protection.
    const minWidth = tabsContainer.style.minWidth;
    const allowsShrinkBelowContent = minWidth === '0' || minWidth === '0px';

    expect(allowsShrinkBelowContent).toBe(false);
  });
});
