/**
 * SplitContainer 2x2 grid resize tests — verifies fix for Bug #524.
 *
 * Bug #524: In a 2x2 split, dragging the horizontal (left/right) divider
 * resizes ALL 4 panes instead of just the 2 adjacent ones.
 *
 * Fix: GridNode with 4 independent ratios and 4 dividers, rendered with
 * absolute positioning. Each divider controls exactly 1 ratio / 2 panes.
 *
 * Test tier: Browser (needs real CSS flexbox + getBoundingClientRect).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { SplitContainer } from './SplitContainer';
import type { GridRatioKey } from '../state/split-types';
import {
  leaf,
  grid,
  createMockPaneMap,
  mountSplitContainer,
  waitForLayout,
} from '../test-utils/browser-split-helpers';
import { maybePromoteToGrid, splitAt } from '../state/split-types';
import type { LayoutNode } from '../state/split-types';

/**
 * Build a 2x2 grid via the real split flow: split right, then split each
 * column down. This triggers maybePromoteToGrid to create a GridNode.
 *
 * Visual:
 *   tl | tr
 *   ---|---
 *   bl | br
 */
function make2x2Tree() {
  return grid(leaf('tl'), leaf('tr'), leaf('bl'), leaf('br'));
}

/**
 * Build a 2x2 grid via the real split + promote flow (same as the app does).
 */
function make2x2TreeViaPromotion(): LayoutNode {
  let tree: LayoutNode = leaf('tl');
  tree = splitAt(tree, 'tl', 'tr', 'horizontal');
  tree = splitAt(tree, 'tl', 'bl', 'vertical');
  tree = splitAt(tree, 'tr', 'br', 'vertical');
  return maybePromoteToGrid(tree);
}

/** Get the bounding rect of a pane by its data-id. */
function getPaneRect(root: HTMLElement, id: string): DOMRect {
  const el = root.querySelector(`[data-id="${id}"]`) as HTMLElement;
  if (!el) throw new Error(`Pane "${id}" not found in DOM`);
  return el.getBoundingClientRect();
}

/** Find all grid dividers of a given direction class. */
function getGridDividers(root: HTMLElement, direction: 'horizontal' | 'vertical'): HTMLElement[] {
  return Array.from(root.querySelectorAll(`.split-grid-divider.${direction}`));
}

/**
 * Simulate a complete divider drag: mousedown → mousemove → mouseup.
 */
function simulateDrag(
  divider: HTMLElement,
  fromX: number,
  fromY: number,
  toX: number,
  toY: number,
) {
  divider.dispatchEvent(new MouseEvent('mousedown', {
    clientX: fromX, clientY: fromY, bubbles: true,
  }));
  document.dispatchEvent(new MouseEvent('mousemove', {
    clientX: toX, clientY: toY,
  }));
  document.dispatchEvent(new MouseEvent('mouseup'));
}

describe('SplitContainer 2x2 grid resize (Bug #524 fix)', () => {
  let onRatioChange: ReturnType<typeof vi.fn>;
  let onGridRatioChange: ReturnType<typeof vi.fn>;
  let onFocusPane: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onRatioChange = vi.fn();
    onGridRatioChange = vi.fn();
    onFocusPane = vi.fn();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    document.head.querySelectorAll('style').forEach(s => s.remove());
  });

  it('maybePromoteToGrid converts H(V,V) to a grid', () => {
    const promoted = make2x2TreeViaPromotion();
    expect(promoted.type).toBe('grid');
    if (promoted.type === 'grid') {
      expect(promoted.children.length).toBe(4);
      expect(promoted.children.map(c => c.type === 'leaf' ? c.terminal_id : null))
        .toEqual(['tl', 'tr', 'bl', 'br']);
    }
  });

  it('renders a 2x2 grid with 4 dividers (2H + 2V)', async () => {
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onGridRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    // 4 panes visible
    expect(root.querySelectorAll('.terminal-pane').length).toBe(4);

    // Grid uses 4 dividers: 2 horizontal (col-resize) + 2 vertical (row-resize)
    const hDividers = getGridDividers(root, 'horizontal');
    const vDividers = getGridDividers(root, 'vertical');
    expect(hDividers.length).toBe(2);
    expect(vDividers.length).toBe(2);

    cleanup();
  });

  it('top row horizontal divider drag only resizes TL and TR (Bug #524 fix)', async () => {
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onGridRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    // Record initial widths
    const initialTL = getPaneRect(root, 'tl');
    const initialTR = getPaneRect(root, 'tr');
    const initialBL = getPaneRect(root, 'bl');
    const initialBR = getPaneRect(root, 'br');

    // Find the top-row horizontal divider (gridKey=col0)
    const hDividers = getGridDividers(root, 'horizontal');
    const topHDivider = hDividers.find(d => d.dataset.gridKey === 'col0')!;
    expect(topHDivider).toBeDefined();

    // Get the grid container rect
    const gridContainer = topHDivider.parentElement!;
    const gridRect = gridContainer.getBoundingClientRect();

    // Drag the top-row horizontal divider to the right (50% → 65%)
    const startX = gridRect.left + gridRect.width * 0.5;
    const dragY = gridRect.top + gridRect.height * 0.25;
    const endX = gridRect.left + gridRect.width * 0.65;

    simulateDrag(topHDivider, startX, dragY, endX, dragY);
    await waitForLayout();

    const afterTL = getPaneRect(root, 'tl');
    const afterTR = getPaneRect(root, 'tr');
    const afterBL = getPaneRect(root, 'bl');
    const afterBR = getPaneRect(root, 'br');

    // TOP ROW panes changed width
    expect(Math.abs(afterTL.width - initialTL.width)).toBeGreaterThan(1);
    expect(Math.abs(afterTR.width - initialTR.width)).toBeGreaterThan(1);

    // BOTTOM ROW panes did NOT change width (Bug #524 fix verified)
    expect(Math.abs(afterBL.width - initialBL.width)).toBeLessThan(1);
    expect(Math.abs(afterBR.width - initialBR.width)).toBeLessThan(1);

    // onGridRatioChange was called with 'col0'
    expect(onGridRatioChange).toHaveBeenCalledWith([], 'col0', expect.any(Number));

    cleanup();
  });

  it('bottom row horizontal divider drag only resizes BL and BR (Bug #524 fix)', async () => {
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onGridRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    const initialTL = getPaneRect(root, 'tl');
    const initialTR = getPaneRect(root, 'tr');
    const initialBL = getPaneRect(root, 'bl');
    const initialBR = getPaneRect(root, 'br');

    // Find the bottom-row horizontal divider (gridKey=col1)
    const hDividers = getGridDividers(root, 'horizontal');
    const bottomHDivider = hDividers.find(d => d.dataset.gridKey === 'col1')!;
    expect(bottomHDivider).toBeDefined();

    const gridContainer = bottomHDivider.parentElement!;
    const gridRect = gridContainer.getBoundingClientRect();

    // Drag the bottom-row horizontal divider to the left (50% → 35%)
    const startX = gridRect.left + gridRect.width * 0.5;
    const dragY = gridRect.top + gridRect.height * 0.75;
    const endX = gridRect.left + gridRect.width * 0.35;

    simulateDrag(bottomHDivider, startX, dragY, endX, dragY);
    await waitForLayout();

    const afterTL = getPaneRect(root, 'tl');
    const afterTR = getPaneRect(root, 'tr');
    const afterBL = getPaneRect(root, 'bl');
    const afterBR = getPaneRect(root, 'br');

    // BOTTOM ROW panes changed width
    expect(Math.abs(afterBL.width - initialBL.width)).toBeGreaterThan(1);
    expect(Math.abs(afterBR.width - initialBR.width)).toBeGreaterThan(1);

    // TOP ROW panes did NOT change width (Bug #524 fix verified)
    expect(Math.abs(afterTL.width - initialTL.width)).toBeLessThan(1);
    expect(Math.abs(afterTR.width - initialTR.width)).toBeLessThan(1);

    // onGridRatioChange was called with 'col1'
    expect(onGridRatioChange).toHaveBeenCalledWith([], 'col1', expect.any(Number));

    cleanup();
  });

  it('left column vertical divider drag only resizes TL and BL', async () => {
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onGridRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    const initialTL = getPaneRect(root, 'tl');
    const initialTR = getPaneRect(root, 'tr');
    const initialBL = getPaneRect(root, 'bl');
    const initialBR = getPaneRect(root, 'br');

    // Find the left-column vertical divider (gridKey=row0)
    const vDividers = getGridDividers(root, 'vertical');
    const leftVDivider = vDividers.find(d => d.dataset.gridKey === 'row0')!;
    expect(leftVDivider).toBeDefined();

    const gridContainer = leftVDivider.parentElement!;
    const gridRect = gridContainer.getBoundingClientRect();

    // Drag the left vertical divider down (50% → 70%)
    const dragX = gridRect.left + gridRect.width * 0.25;
    const startY = gridRect.top + gridRect.height * 0.5;
    const endY = gridRect.top + gridRect.height * 0.7;

    simulateDrag(leftVDivider, dragX, startY, dragX, endY);
    await waitForLayout();

    const afterTL = getPaneRect(root, 'tl');
    const afterTR = getPaneRect(root, 'tr');
    const afterBL = getPaneRect(root, 'bl');
    const afterBR = getPaneRect(root, 'br');

    // LEFT COLUMN panes changed height
    expect(Math.abs(afterTL.height - initialTL.height)).toBeGreaterThan(1);
    expect(Math.abs(afterBL.height - initialBL.height)).toBeGreaterThan(1);

    // RIGHT COLUMN panes did NOT change height
    expect(Math.abs(afterTR.height - initialTR.height)).toBeLessThan(1);
    expect(Math.abs(afterBR.height - initialBR.height)).toBeLessThan(1);

    cleanup();
  });

  it('all 4 dividers are independent — each drag affects exactly 2 panes', async () => {
    // This test verifies the core fix: each divider in the grid is independent.
    const dividerKeys: GridRatioKey[] = ['col0', 'col1', 'row0', 'row1'];
    const affectedPanes: Record<GridRatioKey, { changed: string[]; unchanged: string[] }> = {
      col0: { changed: ['tl', 'tr'], unchanged: ['bl', 'br'] },
      col1: { changed: ['bl', 'br'], unchanged: ['tl', 'tr'] },
      row0: { changed: ['tl', 'bl'], unchanged: ['tr', 'br'] },
      row1: { changed: ['tr', 'br'], unchanged: ['tl', 'bl'] },
    };

    for (const key of dividerKeys) {
      // Create fresh container for each test
      document.body.innerHTML = '';
      document.head.querySelectorAll('style').forEach(s => s.remove());

      const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
      const localOnGridRatioChange = vi.fn();
      const sc = new SplitContainer(make2x2Tree(), {
        paneMap, onRatioChange, onGridRatioChange: localOnGridRatioChange,
        onFocusPane, focusedTerminalId: 'tl',
      });

      const { root, cleanup } = mountSplitContainer(sc);
      await waitForLayout();

      const initial: Record<string, DOMRect> = {};
      for (const id of ['tl', 'tr', 'bl', 'br']) {
        initial[id] = getPaneRect(root, id);
      }

      // Find the divider by its data-gridKey
      const isH = key.startsWith('col');
      const direction = isH ? 'horizontal' : 'vertical';
      const dividers = getGridDividers(root, direction);
      const divider = dividers.find(d => d.dataset.gridKey === key)!;
      expect(divider).toBeDefined();

      const gridRect = divider.parentElement!.getBoundingClientRect();

      // Drag in the appropriate direction
      if (isH) {
        const startX = gridRect.left + gridRect.width * 0.5;
        const midY = gridRect.top + gridRect.height * (key === 'col0' ? 0.25 : 0.75);
        simulateDrag(divider, startX, midY, startX + gridRect.width * 0.15, midY);
      } else {
        const startY = gridRect.top + gridRect.height * 0.5;
        const midX = gridRect.left + gridRect.width * (key === 'row0' ? 0.25 : 0.75);
        simulateDrag(divider, midX, startY, midX, startY + gridRect.height * 0.15);
      }
      await waitForLayout();

      // Check which panes changed
      const measure = isH ? 'width' : 'height';
      for (const id of affectedPanes[key].changed) {
        const after = getPaneRect(root, id);
        expect(
          Math.abs(after[measure] - initial[id][measure]),
          `Divider ${key}: pane "${id}" should have changed ${measure}`,
        ).toBeGreaterThan(1);
      }
      for (const id of affectedPanes[key].unchanged) {
        const after = getPaneRect(root, id);
        expect(
          Math.abs(after[measure] - initial[id][measure]),
          `Divider ${key}: pane "${id}" should NOT have changed ${measure}`,
        ).toBeLessThan(1);
      }

      cleanup();
    }
  });
});
