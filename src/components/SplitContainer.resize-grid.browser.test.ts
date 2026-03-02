/**
 * SplitContainer 2x2 grid resize tests — reproduces Bug #524.
 *
 * Bug #524: In a 2x2 split, dragging the horizontal (left/right) divider
 * resizes ALL 4 panes instead of just the 2 adjacent ones. Vertical
 * (up/down) resize correctly only affects 2 panes.
 *
 * The root cause is the binary tree structure: a 2x2 grid built as
 * H(V(A,C), V(B,D)) has a single root horizontal divider that controls
 * the entire left vs right column width. There are no independent per-row
 * horizontal dividers.
 *
 * Test tier: Browser (needs real CSS flexbox + getBoundingClientRect).
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { SplitContainer } from './SplitContainer';
import {
  leaf,
  split,
  createMockPaneMap,
  mountSplitContainer,
  waitForLayout,
} from '../test-utils/browser-split-helpers';

/**
 * Build a 2x2 grid tree matching the real split order: split right, then
 * split each pane down.
 *
 * Tree: H(V(topLeft, bottomLeft), V(topRight, bottomRight))
 *
 * Visual:
 *   topLeft    | topRight
 *   -----------|----------
 *   bottomLeft | bottomRight
 */
function make2x2Tree() {
  return split('horizontal',
    split('vertical', leaf('tl'), leaf('bl'), 0.5),
    split('vertical', leaf('tr'), leaf('br'), 0.5),
    0.5,
  );
}

/** Get the bounding rect of a pane by its data-id. */
function getPaneRect(root: HTMLElement, id: string): DOMRect {
  const el = root.querySelector(`[data-id="${id}"]`) as HTMLElement;
  if (!el) throw new Error(`Pane "${id}" not found in DOM`);
  return el.getBoundingClientRect();
}

/** Find all dividers of a given direction class. */
function getDividers(root: HTMLElement, direction: 'horizontal' | 'vertical'): HTMLElement[] {
  return Array.from(root.querySelectorAll(`.split-divider.${direction}`));
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

describe('SplitContainer 2x2 grid resize (Bug #524)', () => {
  let onRatioChange: ReturnType<typeof vi.fn>;
  let onFocusPane: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onRatioChange = vi.fn();
    onFocusPane = vi.fn();
  });

  afterEach(() => {
    document.body.innerHTML = '';
    document.head.querySelectorAll('style').forEach(s => s.remove());
  });

  it('renders a 2x2 grid with 3 dividers', async () => {
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    // 4 panes visible
    expect(root.querySelectorAll('.terminal-pane').length).toBe(4);

    // 1 horizontal (root) + 2 vertical (one per column) = 3 dividers total
    const hDividers = getDividers(root, 'horizontal');
    const vDividers = getDividers(root, 'vertical');
    expect(hDividers.length).toBe(1);
    expect(vDividers.length).toBe(2);

    cleanup();
  });

  it('vertical divider drag only resizes 2 adjacent panes (correct behavior)', async () => {
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    // Record initial heights
    const initialTL = getPaneRect(root, 'tl');
    const initialBL = getPaneRect(root, 'bl');
    const initialTR = getPaneRect(root, 'tr');
    const initialBR = getPaneRect(root, 'br');

    // Find the left column's vertical divider (first one in DOM order)
    const vDividers = getDividers(root, 'vertical');
    const leftVDivider = vDividers[0];
    const leftColumnRect = leftVDivider.parentElement!.getBoundingClientRect();

    // Drag the left vertical divider down by 20% of the column height
    const startX = leftColumnRect.left + leftColumnRect.width / 2;
    const startY = leftColumnRect.top + leftColumnRect.height * 0.5;
    const endY = leftColumnRect.top + leftColumnRect.height * 0.7;

    simulateDrag(leftVDivider, startX, startY, startX, endY);
    await waitForLayout();

    // After drag: top-left and bottom-left heights should change
    const afterTL = getPaneRect(root, 'tl');
    const afterBL = getPaneRect(root, 'bl');
    const afterTR = getPaneRect(root, 'tr');
    const afterBR = getPaneRect(root, 'br');

    // Left column panes changed height
    expect(Math.abs(afterTL.height - initialTL.height)).toBeGreaterThan(1);
    expect(Math.abs(afterBL.height - initialBL.height)).toBeGreaterThan(1);

    // Right column panes did NOT change height — correctly scoped
    expect(Math.abs(afterTR.height - initialTR.height)).toBeLessThan(1);
    expect(Math.abs(afterBR.height - initialBR.height)).toBeLessThan(1);

    cleanup();
  });

  it('horizontal divider drag should only resize 2 adjacent panes, not all 4 (Bug #524)', async () => {
    // Bug #524: Dragging the horizontal (col-resize) divider in a 2x2 grid
    // resizes ALL 4 panes. It should only resize the 2 panes in the same
    // row as the drag point.
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    // Record initial widths of all 4 panes
    const initialTL = getPaneRect(root, 'tl');
    const initialTR = getPaneRect(root, 'tr');
    const initialBL = getPaneRect(root, 'bl');
    const initialBR = getPaneRect(root, 'br');

    // All 4 panes should start at ~50% width each (within their columns)
    // At 800px root width, each column is ~399px (minus 2px divider)
    expect(Math.abs(initialTL.width - initialTR.width)).toBeLessThan(5);
    expect(Math.abs(initialBL.width - initialBR.width)).toBeLessThan(5);

    // Find the root horizontal divider (there's only 1)
    const hDividers = getDividers(root, 'horizontal');
    expect(hDividers.length).toBe(1);
    const hDivider = hDividers[0];

    // The root container is the divider's parent
    const rootContainer = hDivider.parentElement!;
    const rootRect = rootContainer.getBoundingClientRect();

    // Drag the horizontal divider to the right — from 50% to 65%
    // We drag at the TOP HALF of the divider (near the top row)
    const startX = rootRect.left + rootRect.width * 0.5;
    const dragY = rootRect.top + rootRect.height * 0.25; // top quarter
    const endX = rootRect.left + rootRect.width * 0.65;

    simulateDrag(hDivider, startX, dragY, endX, dragY);
    await waitForLayout();

    // After drag: measure all pane widths
    const afterTL = getPaneRect(root, 'tl');
    const afterTR = getPaneRect(root, 'tr');
    const afterBL = getPaneRect(root, 'bl');
    const afterBR = getPaneRect(root, 'br');

    // TOP ROW panes changed width (this is near where we dragged)
    const topLeftWidthDelta = Math.abs(afterTL.width - initialTL.width);
    const topRightWidthDelta = Math.abs(afterTR.width - initialTR.width);
    expect(topLeftWidthDelta).toBeGreaterThan(1);
    expect(topRightWidthDelta).toBeGreaterThan(1);

    // BOTTOM ROW panes should NOT have changed width
    // Bug #524: They DO change, because the root H-split affects all panes
    const bottomLeftWidthDelta = Math.abs(afterBL.width - initialBL.width);
    const bottomRightWidthDelta = Math.abs(afterBR.width - initialBR.width);
    expect(bottomLeftWidthDelta).toBeLessThan(1);
    expect(bottomRightWidthDelta).toBeLessThan(1);

    cleanup();
  });

  it('dragging a horizontal divider at the bottom row should not affect top row (Bug #524)', async () => {
    // Complementary test: drag from the bottom half, verify top row unaffected
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    const initialTL = getPaneRect(root, 'tl');
    const initialTR = getPaneRect(root, 'tr');
    const initialBL = getPaneRect(root, 'bl');
    const initialBR = getPaneRect(root, 'br');

    const hDivider = getDividers(root, 'horizontal')[0];
    const rootRect = hDivider.parentElement!.getBoundingClientRect();

    // Drag at the BOTTOM HALF (near the bottom row)
    const startX = rootRect.left + rootRect.width * 0.5;
    const dragY = rootRect.top + rootRect.height * 0.75; // bottom quarter
    const endX = rootRect.left + rootRect.width * 0.35;  // drag left to 35%

    simulateDrag(hDivider, startX, dragY, endX, dragY);
    await waitForLayout();

    const afterTL = getPaneRect(root, 'tl');
    const afterTR = getPaneRect(root, 'tr');
    const afterBL = getPaneRect(root, 'bl');
    const afterBR = getPaneRect(root, 'br');

    // Bottom row panes should change (we dragged near them)
    expect(Math.abs(afterBL.width - initialBL.width)).toBeGreaterThan(1);
    expect(Math.abs(afterBR.width - initialBR.width)).toBeGreaterThan(1);

    // Top row panes should NOT change
    // Bug #524: They DO change because the root H-split affects everything
    expect(Math.abs(afterTL.width - initialTL.width)).toBeLessThan(1);
    expect(Math.abs(afterTR.width - initialTR.width)).toBeLessThan(1);

    cleanup();
  });

  it('demonstrates the asymmetry: V-resize is scoped but H-resize is global (Bug #524)', async () => {
    // This test explicitly shows the asymmetry between vertical and horizontal
    // resize behavior in a 2x2 grid.
    const paneMap = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc = new SplitContainer(make2x2Tree(), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    await waitForLayout();

    // --- Part 1: Drag a vertical divider (up/down) ---
    const vDividers = getDividers(root, 'vertical');
    const leftVDivider = vDividers[0];
    const leftCol = leftVDivider.parentElement!.getBoundingClientRect();

    const beforeVDrag = {
      tl: getPaneRect(root, 'tl'),
      tr: getPaneRect(root, 'tr'),
      bl: getPaneRect(root, 'bl'),
      br: getPaneRect(root, 'br'),
    };

    simulateDrag(
      leftVDivider,
      leftCol.left + leftCol.width / 2,
      leftCol.top + leftCol.height * 0.5,
      leftCol.left + leftCol.width / 2,
      leftCol.top + leftCol.height * 0.7,
    );
    await waitForLayout();

    const afterVDrag = {
      tl: getPaneRect(root, 'tl'),
      tr: getPaneRect(root, 'tr'),
      bl: getPaneRect(root, 'bl'),
      br: getPaneRect(root, 'br'),
    };

    // Count panes whose height changed
    const vChangedCount = ['tl', 'tr', 'bl', 'br'].filter(id => {
      const before = beforeVDrag[id as keyof typeof beforeVDrag];
      const after = afterVDrag[id as keyof typeof afterVDrag];
      return Math.abs(after.height - before.height) > 1;
    }).length;

    // Vertical resize correctly affects only 2 panes
    expect(vChangedCount).toBe(2);

    // --- Part 2: Reset and drag a horizontal divider (left/right) ---
    // Re-create the container to reset ratios
    sc.destroy();
    document.body.innerHTML = '';

    const paneMap2 = createMockPaneMap(['tl', 'tr', 'bl', 'br']);
    const sc2 = new SplitContainer(make2x2Tree(), {
      paneMap: paneMap2, onRatioChange, onFocusPane, focusedTerminalId: 'tl',
    });
    const { root: root2, cleanup: cleanup2 } = mountSplitContainer(sc2);
    await waitForLayout();

    const hDivider = getDividers(root2, 'horizontal')[0];
    const rootRect = hDivider.parentElement!.getBoundingClientRect();

    const beforeHDrag = {
      tl: getPaneRect(root2, 'tl'),
      tr: getPaneRect(root2, 'tr'),
      bl: getPaneRect(root2, 'bl'),
      br: getPaneRect(root2, 'br'),
    };

    simulateDrag(
      hDivider,
      rootRect.left + rootRect.width * 0.5,
      rootRect.top + rootRect.height * 0.25,
      rootRect.left + rootRect.width * 0.65,
      rootRect.top + rootRect.height * 0.25,
    );
    await waitForLayout();

    const afterHDrag = {
      tl: getPaneRect(root2, 'tl'),
      tr: getPaneRect(root2, 'tr'),
      bl: getPaneRect(root2, 'bl'),
      br: getPaneRect(root2, 'br'),
    };

    // Count panes whose width changed
    const hChangedCount = ['tl', 'tr', 'bl', 'br'].filter(id => {
      const before = beforeHDrag[id as keyof typeof beforeHDrag];
      const after = afterHDrag[id as keyof typeof afterHDrag];
      return Math.abs(after.width - before.width) > 1;
    }).length;

    // Bug #524: Horizontal resize affects ALL 4 panes, but should only affect 2
    // (matching the vertical resize behavior)
    expect(hChangedCount).toBe(2);

    cleanup2();
  });
});
