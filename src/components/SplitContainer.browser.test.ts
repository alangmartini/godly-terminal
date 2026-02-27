/**
 * SplitContainer browser tests — validates layout behavior with real Chromium DOM.
 *
 * Unlike the jsdom unit tests, these get real getBoundingClientRect values,
 * real CSS flexbox layout, and real pointer events.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { SplitContainer } from './SplitContainer';
import {
  leaf,
  split,
  createMockPaneMap,
  mountSplitContainer,
} from '../test-utils/browser-split-helpers';

describe('SplitContainer (browser)', () => {
  let onRatioChange: ReturnType<typeof vi.fn>;
  let onFocusPane: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onRatioChange = vi.fn();
    onFocusPane = vi.fn();
  });

  afterEach(() => {
    // Clean up DOM
    document.body.innerHTML = '';
    document.head.querySelectorAll('style').forEach(s => s.remove());
  });

  it('renders a single leaf pane into the DOM', () => {
    const paneMap = createMockPaneMap(['t1']);
    const sc = new SplitContainer(leaf('t1'), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
    });

    const { root, cleanup } = mountSplitContainer(sc);
    expect(root.querySelector('[data-id="t1"]')).not.toBeNull();
    cleanup();
  });

  it('creates a horizontal split with real flex layout', () => {
    const paneMap = createMockPaneMap(['t1', 't2']);
    const sc = new SplitContainer(
      split('horizontal', leaf('t1'), leaf('t2'), 0.5),
      { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
    );

    const { root, cleanup } = mountSplitContainer(sc);

    const container = root.querySelector('.split-container.horizontal');
    expect(container).not.toBeNull();
    expect(container!.children.length).toBe(3); // pane, divider, pane

    // In real Chromium, flex layout produces actual dimensions
    const rootRect = root.getBoundingClientRect();
    expect(rootRect.width).toBeGreaterThan(0);
    expect(rootRect.height).toBeGreaterThan(0);
    cleanup();
  });

  it('creates a vertical split', () => {
    const paneMap = createMockPaneMap(['t1', 't2']);
    const sc = new SplitContainer(
      split('vertical', leaf('t1'), leaf('t2')),
      { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
    );

    const { root, cleanup } = mountSplitContainer(sc);
    const container = root.querySelector('.split-container.vertical');
    expect(container).not.toBeNull();
    cleanup();
  });

  it('renders nested 3-pane layout with real bounding rects', () => {
    const tree = split('horizontal',
      leaf('t1'),
      split('vertical', leaf('t2'), leaf('t3')),
    );
    const paneMap = createMockPaneMap(['t1', 't2', 't3']);
    const sc = new SplitContainer(tree, {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
    });

    const { root, cleanup } = mountSplitContainer(sc);

    // All 3 panes should be in the DOM
    expect(root.querySelectorAll('.terminal-pane').length).toBe(3);

    // Dividers should have real positions
    const dividers = root.querySelectorAll('.split-divider');
    expect(dividers.length).toBe(2);
    for (const divider of dividers) {
      const rect = divider.getBoundingClientRect();
      // In real Chromium, dividers get actual layout — not zeros like jsdom
      expect(rect.width + rect.height).toBeGreaterThan(0);
    }
    cleanup();
  });

  it('updates focus without re-rendering structure', () => {
    const paneMap = createMockPaneMap(['t1', 't2']);
    const sc = new SplitContainer(
      split('horizontal', leaf('t1'), leaf('t2')),
      { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
    );

    const { cleanup } = mountSplitContainer(sc);

    vi.mocked(paneMap.get('t1')!.setSplitVisible).mockClear();
    vi.mocked(paneMap.get('t2')!.setSplitVisible).mockClear();

    sc.updateFocus('t2');

    expect(paneMap.get('t1')!.setSplitVisible).toHaveBeenCalledWith(true, false);
    expect(paneMap.get('t2')!.setSplitVisible).toHaveBeenCalledWith(true, true);
    cleanup();
  });

  it('handles divider drag with real getBoundingClientRect', () => {
    const paneMap = createMockPaneMap(['t1', 't2']);
    const sc = new SplitContainer(
      split('horizontal', leaf('t1'), leaf('t2'), 0.5),
      { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
    );

    const { root, cleanup } = mountSplitContainer(sc);
    const divider = root.querySelector('.split-divider') as HTMLElement;
    expect(divider).not.toBeNull();

    // Get real container rect from Chromium layout
    const container = divider.parentElement!;
    const rect = container.getBoundingClientRect();

    // Simulate drag: mousedown on divider, then mousemove to 60% position
    divider.dispatchEvent(new MouseEvent('mousedown', {
      clientX: rect.left + rect.width * 0.5,
      clientY: rect.top + rect.height * 0.5,
      bubbles: true,
    }));

    document.dispatchEvent(new MouseEvent('mousemove', {
      clientX: rect.left + rect.width * 0.6,
      clientY: rect.top + rect.height * 0.5,
    }));

    expect(onRatioChange).toHaveBeenCalled();
    const [, ratio] = onRatioChange.mock.calls[0];
    expect(ratio).toBeGreaterThan(0.15);
    expect(ratio).toBeLessThan(0.85);

    // Clean up: mouseup
    document.dispatchEvent(new MouseEvent('mouseup'));
    cleanup();
  });

  it('destroy removes element from DOM', () => {
    const paneMap = createMockPaneMap(['t1']);
    const sc = new SplitContainer(leaf('t1'), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
    });

    mountSplitContainer(sc);
    expect(document.body.querySelector('.split-root')).not.toBeNull();

    sc.destroy();
    expect(document.body.querySelector('.split-root')).toBeNull();
  });
});
