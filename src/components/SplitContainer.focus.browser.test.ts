/**
 * SplitContainer focus navigation browser tests.
 *
 * Tests the focus navigation pipeline: findAdjacentTerminal() → sc.updateFocus()
 * → setSplitVisible calls. This is the DOM integration seam that App.ts keyboard
 * handlers use on Ctrl+Arrow.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { SplitContainer } from './SplitContainer';
import { findAdjacentTerminal } from '../state/split-types';
import {
  leaf,
  split,
  createMockPaneMap,
  mountSplitContainer,
  assertFocusedPane,
} from '../test-utils/browser-split-helpers';

afterEach(() => {
  document.body.innerHTML = '';
  document.head.querySelectorAll('style').forEach(s => s.remove());
});

// ---------------------------------------------------------------------------
// 2-pane horizontal split [t1 | t2]
// ---------------------------------------------------------------------------

describe('2-pane horizontal [t1 | t2]', () => {
  function setup(focusedId: string) {
    const paneMap = createMockPaneMap(['t1', 't2']);
    const layout = split('horizontal', leaf('t1'), leaf('t2'));
    const sc = new SplitContainer(layout, {
      paneMap, onRatioChange: vi.fn(), onFocusPane: vi.fn(),
      focusedTerminalId: focusedId,
    });
    const { cleanup } = mountSplitContainer(sc);
    // Clear constructor calls so assertions only see updateFocus calls
    for (const [, pane] of paneMap) vi.mocked(pane.setSplitVisible).mockClear();
    return { layout, paneMap, sc, cleanup };
  }

  it('focus right: t1 → t2', () => {
    const { layout, paneMap, sc, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'horizontal', true);
    expect(adjacent).toBe('t2');
    sc.updateFocus('t2');
    assertFocusedPane(paneMap, 't2');
    cleanup();
  });

  it('focus left: t2 → t1', () => {
    const { layout, paneMap, sc, cleanup } = setup('t2');
    const adjacent = findAdjacentTerminal(layout, 't2', 'horizontal', false);
    expect(adjacent).toBe('t1');
    sc.updateFocus('t1');
    assertFocusedPane(paneMap, 't1');
    cleanup();
  });

  it('focus up from horizontal: no-op', () => {
    const { layout, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'vertical', false);
    expect(adjacent).toBeNull();
    cleanup();
  });

  it('focus down from horizontal: no-op', () => {
    const { layout, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'vertical', true);
    expect(adjacent).toBeNull();
    cleanup();
  });

  it('boundary: focus right at rightmost stays put', () => {
    const { layout, cleanup } = setup('t2');
    const adjacent = findAdjacentTerminal(layout, 't2', 'horizontal', true);
    expect(adjacent).toBeNull();
    cleanup();
  });
});

// ---------------------------------------------------------------------------
// 2-pane vertical split [t1 / t2]
// ---------------------------------------------------------------------------

describe('2-pane vertical [t1 / t2]', () => {
  function setup(focusedId: string) {
    const paneMap = createMockPaneMap(['t1', 't2']);
    const layout = split('vertical', leaf('t1'), leaf('t2'));
    const sc = new SplitContainer(layout, {
      paneMap, onRatioChange: vi.fn(), onFocusPane: vi.fn(),
      focusedTerminalId: focusedId,
    });
    const { cleanup } = mountSplitContainer(sc);
    for (const [, pane] of paneMap) vi.mocked(pane.setSplitVisible).mockClear();
    return { layout, paneMap, sc, cleanup };
  }

  it('focus down: t1 → t2', () => {
    const { layout, paneMap, sc, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'vertical', true);
    expect(adjacent).toBe('t2');
    sc.updateFocus('t2');
    assertFocusedPane(paneMap, 't2');
    cleanup();
  });

  it('focus up: t2 → t1', () => {
    const { layout, paneMap, sc, cleanup } = setup('t2');
    const adjacent = findAdjacentTerminal(layout, 't2', 'vertical', false);
    expect(adjacent).toBe('t1');
    sc.updateFocus('t1');
    assertFocusedPane(paneMap, 't1');
    cleanup();
  });

  it('focus left from vertical: no-op', () => {
    const { layout, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'horizontal', false);
    expect(adjacent).toBeNull();
    cleanup();
  });

  it('focus right from vertical: no-op', () => {
    const { layout, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'horizontal', true);
    expect(adjacent).toBeNull();
    cleanup();
  });
});

// ---------------------------------------------------------------------------
// 4-pane grid (horizontal of two verticals)
//   [t1] [t3]
//   [t2] [t4]
// ---------------------------------------------------------------------------

describe('4-pane grid [t1|t3 / t2|t4]', () => {
  function setup(focusedId: string) {
    const paneMap = createMockPaneMap(['t1', 't2', 't3', 't4']);
    const layout = split('horizontal',
      split('vertical', leaf('t1'), leaf('t2')),
      split('vertical', leaf('t3'), leaf('t4')),
    );
    const sc = new SplitContainer(layout, {
      paneMap, onRatioChange: vi.fn(), onFocusPane: vi.fn(),
      focusedTerminalId: focusedId,
    });
    const { cleanup } = mountSplitContainer(sc);
    for (const [, pane] of paneMap) vi.mocked(pane.setSplitVisible).mockClear();
    return { layout, paneMap, sc, cleanup };
  }

  it('right from t1 → t3', () => {
    const { layout, paneMap, sc, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'horizontal', true);
    expect(adjacent).toBe('t3');
    sc.updateFocus('t3');
    assertFocusedPane(paneMap, 't3');
    cleanup();
  });

  it('down from t1 → t2', () => {
    const { layout, paneMap, sc, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'vertical', true);
    expect(adjacent).toBe('t2');
    sc.updateFocus('t2');
    assertFocusedPane(paneMap, 't2');
    cleanup();
  });

  it('right from t2 → t3 (nearest leaf in adjacent subtree)', () => {
    const { layout, paneMap, sc, cleanup } = setup('t2');
    // findAdjacentTerminal returns firstLeaf of the right column, not grid-aligned
    const adjacent = findAdjacentTerminal(layout, 't2', 'horizontal', true);
    expect(adjacent).toBe('t3');
    sc.updateFocus('t3');
    assertFocusedPane(paneMap, 't3');
    cleanup();
  });

  it('up from t4 → t3', () => {
    const { layout, paneMap, sc, cleanup } = setup('t4');
    const adjacent = findAdjacentTerminal(layout, 't4', 'vertical', false);
    expect(adjacent).toBe('t3');
    sc.updateFocus('t3');
    assertFocusedPane(paneMap, 't3');
    cleanup();
  });

  it('left from t3 → t2 (lastLeaf of adjacent subtree)', () => {
    const { layout, paneMap, sc, cleanup } = setup('t3');
    // findAdjacentTerminal returns lastLeaf of the left column, not grid-aligned
    const adjacent = findAdjacentTerminal(layout, 't3', 'horizontal', false);
    expect(adjacent).toBe('t2');
    sc.updateFocus('t2');
    assertFocusedPane(paneMap, 't2');
    cleanup();
  });

  it('boundary: up from t1 stays put', () => {
    const { layout, cleanup } = setup('t1');
    const adjacent = findAdjacentTerminal(layout, 't1', 'vertical', false);
    expect(adjacent).toBeNull();
    cleanup();
  });

  it('boundary: right from t4 stays put', () => {
    const { layout, cleanup } = setup('t4');
    const adjacent = findAdjacentTerminal(layout, 't4', 'horizontal', true);
    expect(adjacent).toBeNull();
    cleanup();
  });
});

// ---------------------------------------------------------------------------
// Focus after structural changes
// ---------------------------------------------------------------------------

describe('focus after structural changes', () => {
  it('focus correct after adding a new split', () => {
    const paneMap = createMockPaneMap(['t1', 't2', 't3']);
    const initialLayout = split('horizontal', leaf('t1'), leaf('t2'));
    const sc = new SplitContainer(initialLayout, {
      paneMap, onRatioChange: vi.fn(), onFocusPane: vi.fn(),
      focusedTerminalId: 't1',
    });
    const { cleanup } = mountSplitContainer(sc);

    // Add a vertical split on the right side
    const newLayout = split('horizontal',
      leaf('t1'),
      split('vertical', leaf('t2'), leaf('t3')),
    );
    sc.update(newLayout, 't3');

    // Clear and re-check
    for (const [, pane] of paneMap) vi.mocked(pane.setSplitVisible).mockClear();
    sc.updateFocus('t3');
    assertFocusedPane(paneMap, 't3');
    cleanup();
  });

  it('focus moves to sibling when focused pane removed', () => {
    const paneMap = createMockPaneMap(['t1', 't2']);
    const layout = split('horizontal', leaf('t1'), leaf('t2'));
    const sc = new SplitContainer(layout, {
      paneMap, onRatioChange: vi.fn(), onFocusPane: vi.fn(),
      focusedTerminalId: 't2',
    });
    const { cleanup } = mountSplitContainer(sc);

    // Remove t2: layout collapses to just t1
    sc.update(leaf('t1'), 't1');

    for (const [, pane] of paneMap) vi.mocked(pane.setSplitVisible).mockClear();
    sc.updateFocus('t1');

    // Only t1 should get setSplitVisible calls now
    const t1Mock = vi.mocked(paneMap.get('t1')!.setSplitVisible);
    const lastCall = t1Mock.mock.calls[t1Mock.mock.calls.length - 1];
    expect(lastCall).toEqual([true, true]);
    cleanup();
  });
});

// ---------------------------------------------------------------------------
// DOM correctness
// ---------------------------------------------------------------------------

describe('DOM correctness on focus change', () => {
  it('no structural DOM rebuild on focus change', () => {
    const paneMap = createMockPaneMap(['t1', 't2']);
    const layout = split('horizontal', leaf('t1'), leaf('t2'));
    const sc = new SplitContainer(layout, {
      paneMap, onRatioChange: vi.fn(), onFocusPane: vi.fn(),
      focusedTerminalId: 't1',
    });
    const { root, cleanup } = mountSplitContainer(sc);

    // Capture element references before focus change
    const pane1Before = root.querySelector('[data-id="t1"]');
    const pane2Before = root.querySelector('[data-id="t2"]');
    const dividerBefore = root.querySelector('.split-divider');

    sc.updateFocus('t2');

    // Same DOM elements should still be in place (no rebuild)
    expect(root.querySelector('[data-id="t1"]')).toBe(pane1Before);
    expect(root.querySelector('[data-id="t2"]')).toBe(pane2Before);
    expect(root.querySelector('.split-divider')).toBe(dividerBefore);
    cleanup();
  });

  it('setSplitVisible(true, true) called for focused pane only', () => {
    const paneMap = createMockPaneMap(['t1', 't2', 't3']);
    const layout = split('horizontal',
      leaf('t1'),
      split('vertical', leaf('t2'), leaf('t3')),
    );
    const sc = new SplitContainer(layout, {
      paneMap, onRatioChange: vi.fn(), onFocusPane: vi.fn(),
      focusedTerminalId: 't1',
    });
    const { cleanup } = mountSplitContainer(sc);

    for (const [, pane] of paneMap) vi.mocked(pane.setSplitVisible).mockClear();
    sc.updateFocus('t2');

    // t2 focused
    expect(paneMap.get('t2')!.setSplitVisible).toHaveBeenCalledWith(true, true);
    // t1 and t3 visible but not focused
    expect(paneMap.get('t1')!.setSplitVisible).toHaveBeenCalledWith(true, false);
    expect(paneMap.get('t3')!.setSplitVisible).toHaveBeenCalledWith(true, false);
    cleanup();
  });
});
