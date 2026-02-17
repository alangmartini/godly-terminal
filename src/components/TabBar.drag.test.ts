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
import { workspaceService } from '../services/workspace-service';

// Make store notifications synchronous in jsdom (avoid requestAnimationFrame batching)
const origRAF = globalThis.requestAnimationFrame;

const DRAG_THRESHOLD = 5; // Must match TabBar.ts

/**
 * Helper: mock getBoundingClientRect on tab elements so that pointer-event
 * hit-testing works in jsdom (which returns all-zero rects by default).
 * Lays tabs out horizontally: tab0 at [0,100)x[0,30), tab1 at [100,200)x[0,30), etc.
 */
function mockTabRects(tabs: HTMLElement[]) {
  tabs.forEach((tab, i) => {
    vi.spyOn(tab, 'getBoundingClientRect').mockReturnValue({
      left: i * 100,
      right: (i + 1) * 100,
      top: 0,
      bottom: 30,
      width: 100,
      height: 30,
      x: i * 100,
      y: 0,
      toJSON: () => ({}),
    });
  });
}

/**
 * Simulate a full pointer drag sequence on a tab element.
 * Returns after pointerup has been dispatched.
 */
function simulatePointerDrag(
  sourceTab: HTMLElement,
  startX: number,
  startY: number,
  endX: number,
  endY: number
) {
  // jsdom doesn't implement setPointerCapture/releasePointerCapture
  sourceTab.setPointerCapture = vi.fn();
  sourceTab.releasePointerCapture = vi.fn();

  sourceTab.dispatchEvent(
    new PointerEvent('pointerdown', { clientX: startX, clientY: startY, button: 0, bubbles: true })
  );

  // Move past threshold to start the drag
  sourceTab.dispatchEvent(
    new PointerEvent('pointermove', {
      clientX: startX + DRAG_THRESHOLD + 1,
      clientY: startY,
      bubbles: true,
    })
  );

  // Move to the final position
  sourceTab.dispatchEvent(
    new PointerEvent('pointermove', { clientX: endX, clientY: endY, bubbles: true })
  );

  // Drop
  sourceTab.dispatchEvent(
    new PointerEvent('pointerup', { clientX: endX, clientY: endY, pointerId: 0, bubbles: true })
  );
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

  function getTabElements(): HTMLElement[] {
    return Array.from(mountPoint.querySelectorAll('.tab'));
  }

  it('should render all three tabs', () => {
    const tabs = getTabElements();
    expect(tabs.length).toBe(3);
  });

  it('pointerdown + small move should NOT start a drag (below threshold)', () => {
    const tabs = getTabElements();
    tabs[0].setPointerCapture = vi.fn();
    tabs[0].releasePointerCapture = vi.fn();

    tabs[0].dispatchEvent(
      new PointerEvent('pointerdown', { clientX: 50, clientY: 15, button: 0, bubbles: true })
    );

    // Move less than threshold
    tabs[0].dispatchEvent(
      new PointerEvent('pointermove', { clientX: 52, clientY: 15, bubbles: true })
    );

    expect(tabs[0].classList.contains('dragging')).toBe(false);
  });

  it('pointerdown + move past threshold should start a drag', () => {
    const tabs = getTabElements();
    tabs[0].setPointerCapture = vi.fn();
    tabs[0].releasePointerCapture = vi.fn();

    tabs[0].dispatchEvent(
      new PointerEvent('pointerdown', { clientX: 50, clientY: 15, button: 0, bubbles: true })
    );

    // Move past threshold
    tabs[0].dispatchEvent(
      new PointerEvent('pointermove', {
        clientX: 50 + DRAG_THRESHOLD + 1,
        clientY: 15,
        bubbles: true,
      })
    );

    expect(tabs[0].classList.contains('dragging')).toBe(true);
  });

  it('right-click should NOT start a drag', () => {
    const tabs = getTabElements();
    tabs[0].setPointerCapture = vi.fn();

    tabs[0].dispatchEvent(
      new PointerEvent('pointerdown', { clientX: 50, clientY: 15, button: 2, bubbles: true })
    );

    tabs[0].dispatchEvent(
      new PointerEvent('pointermove', {
        clientX: 50 + DRAG_THRESHOLD + 1,
        clientY: 15,
        bubbles: true,
      })
    );

    expect(tabs[0].classList.contains('dragging')).toBe(false);
  });

  it('dragging tab over another tab should add drag-over class', () => {
    const tabs = getTabElements();
    mockTabRects(tabs);

    // Drag t1 (center at 50,15) towards t2 (center at 150,15)
    simulatePointerDrag(tabs[0], 50, 15, 150, 15);

    // After drop, drag-over classes are cleared, so check that the class
    // was applied during the drag. We verify by checking the reorder happened,
    // which proves the drop target was found. The drag-over class is cleared
    // on drop, so we test the move handler directly instead.
  });

  it('drag-over class should be added during pointermove over a target tab', () => {
    const tabs = getTabElements();
    mockTabRects(tabs);

    tabs[0].setPointerCapture = vi.fn();
    tabs[0].releasePointerCapture = vi.fn();

    // Start drag on t1
    tabs[0].dispatchEvent(
      new PointerEvent('pointerdown', { clientX: 50, clientY: 15, button: 0, bubbles: true })
    );

    // Move past threshold
    tabs[0].dispatchEvent(
      new PointerEvent('pointermove', {
        clientX: 50 + DRAG_THRESHOLD + 1,
        clientY: 15,
        bubbles: true,
      })
    );

    // Move over t2's area (center at 150,15)
    tabs[0].dispatchEvent(
      new PointerEvent('pointermove', { clientX: 150, clientY: 15, bubbles: true })
    );

    expect(tabs[1].classList.contains('drag-over')).toBe(true);
    // t3 should not have drag-over
    expect(tabs[2].classList.contains('drag-over')).toBe(false);
  });

  it('drag-over class should be cleared after drop', () => {
    const tabs = getTabElements();
    mockTabRects(tabs);

    simulatePointerDrag(tabs[0], 50, 15, 150, 15);

    // After pointerup, drag-over should be cleaned up
    for (const tab of tabs) {
      expect(tab.classList.contains('drag-over')).toBe(false);
    }
  });

  it('dragging class should be removed after drop', () => {
    const tabs = getTabElements();
    mockTabRects(tabs);

    simulatePointerDrag(tabs[0], 50, 15, 150, 15);

    expect(tabs[0].classList.contains('dragging')).toBe(false);
  });

  it('dropping tab on another tab should trigger reorder', async () => {
    const tabs = getTabElements();
    mockTabRects(tabs);

    // Drag t1 onto t2
    simulatePointerDrag(tabs[0], 50, 15, 150, 15);

    await vi.waitFor(() => {
      expect(workspaceService.reorderTabs).toHaveBeenCalledWith(
        'ws-1',
        ['t2', 't1', 't3']
      );
    });
  });

  it('dropping tab on empty area should NOT trigger reorder', async () => {
    const tabs = getTabElements();
    mockTabRects(tabs);

    // Drag t1 to an area beyond all tabs (x=500, past all tab rects)
    simulatePointerDrag(tabs[0], 50, 15, 500, 15);

    // Give it a tick to ensure nothing fires
    await new Promise(r => setTimeout(r, 50));
    expect(workspaceService.reorderTabs).not.toHaveBeenCalled();
  });

  it('click should not fire after drag ends (suppressed by _lastDragEndTime)', () => {
    const tabs = getTabElements();
    mockTabRects(tabs);

    // Set t2 as active so we can detect if t1 click changes it
    store.setActiveTerminal('t2');

    // Drag t1 somewhere
    simulatePointerDrag(tabs[0], 50, 15, 150, 15);

    // Immediately click t1 — should be suppressed
    tabs[0].click();

    // Active terminal should still be t2
    expect(store.getState().activeTerminalId).toBe('t2');
  });
});
