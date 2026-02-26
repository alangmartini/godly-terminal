/**
 * SplitContainer browser tests — validates layout behavior with real Chromium DOM.
 *
 * Unlike the jsdom unit tests, these get real getBoundingClientRect values,
 * real CSS flexbox layout, and real pointer events.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { SplitContainer, SplitPaneHandle } from './SplitContainer';
import { LayoutNode, LeafNode, SplitNode } from '../state/split-types';

function leaf(id: string): LeafNode {
  return { type: 'leaf', terminal_id: id };
}

function split(
  dir: 'horizontal' | 'vertical',
  first: LayoutNode,
  second: LayoutNode,
  ratio = 0.5,
): SplitNode {
  return { type: 'split', direction: dir, ratio, first, second };
}

function mockPane(id: string): SplitPaneHandle {
  const container = document.createElement('div');
  container.className = 'terminal-pane';
  container.dataset.id = id;
  // Give it some default size so flex layout has something to work with
  container.style.minWidth = '50px';
  container.style.minHeight = '50px';
  return {
    getContainer: () => container,
    setSplitVisible: vi.fn(),
    setActive: vi.fn(),
  };
}

function createPaneMap(ids: string[]): Map<string, SplitPaneHandle> {
  const map = new Map<string, SplitPaneHandle>();
  for (const id of ids) {
    map.set(id, mockPane(id));
  }
  return map;
}

/** Mount the split container into the real DOM and inject layout styles. */
function mountWithStyles(sc: SplitContainer): HTMLElement {
  const root = sc.getElement();

  // Inject minimal split layout CSS
  const style = document.createElement('style');
  style.textContent = `
    .split-root { width: 800px; height: 600px; position: relative; }
    .split-container { display: flex; width: 100%; height: 100%; }
    .split-container.horizontal { flex-direction: row; }
    .split-container.vertical { flex-direction: column; }
    .split-divider { flex-shrink: 0; }
    .split-divider.horizontal { width: 2px; cursor: col-resize; }
    .split-divider.vertical { height: 2px; cursor: row-resize; }
    .terminal-pane { overflow: hidden; }
  `;
  document.head.appendChild(style);
  document.body.appendChild(root);

  return root;
}

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
    const paneMap = createPaneMap(['t1']);
    const sc = new SplitContainer(leaf('t1'), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
    });

    const root = mountWithStyles(sc);
    expect(root.querySelector('[data-id="t1"]')).not.toBeNull();
  });

  it('creates a horizontal split with real flex layout', () => {
    const paneMap = createPaneMap(['t1', 't2']);
    const sc = new SplitContainer(
      split('horizontal', leaf('t1'), leaf('t2'), 0.5),
      { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
    );

    const root = mountWithStyles(sc);

    const container = root.querySelector('.split-container.horizontal');
    expect(container).not.toBeNull();
    expect(container!.children.length).toBe(3); // pane, divider, pane

    // In real Chromium, flex layout produces actual dimensions
    const rootRect = root.getBoundingClientRect();
    expect(rootRect.width).toBeGreaterThan(0);
    expect(rootRect.height).toBeGreaterThan(0);
  });

  it('creates a vertical split', () => {
    const paneMap = createPaneMap(['t1', 't2']);
    const sc = new SplitContainer(
      split('vertical', leaf('t1'), leaf('t2')),
      { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
    );

    const root = mountWithStyles(sc);
    const container = root.querySelector('.split-container.vertical');
    expect(container).not.toBeNull();
  });

  it('renders nested 3-pane layout with real bounding rects', () => {
    const tree = split('horizontal',
      leaf('t1'),
      split('vertical', leaf('t2'), leaf('t3')),
    );
    const paneMap = createPaneMap(['t1', 't2', 't3']);
    const sc = new SplitContainer(tree, {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
    });

    const root = mountWithStyles(sc);

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
  });

  it('updates focus without re-rendering structure', () => {
    const paneMap = createPaneMap(['t1', 't2']);
    const sc = new SplitContainer(
      split('horizontal', leaf('t1'), leaf('t2')),
      { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
    );

    mountWithStyles(sc);

    vi.mocked(paneMap.get('t1')!.setSplitVisible).mockClear();
    vi.mocked(paneMap.get('t2')!.setSplitVisible).mockClear();

    sc.updateFocus('t2');

    expect(paneMap.get('t1')!.setSplitVisible).toHaveBeenCalledWith(true, false);
    expect(paneMap.get('t2')!.setSplitVisible).toHaveBeenCalledWith(true, true);
  });

  it('handles divider drag with real getBoundingClientRect', () => {
    const paneMap = createPaneMap(['t1', 't2']);
    const sc = new SplitContainer(
      split('horizontal', leaf('t1'), leaf('t2'), 0.5),
      { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
    );

    const root = mountWithStyles(sc);
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
  });

  it('destroy removes element from DOM', () => {
    const paneMap = createPaneMap(['t1']);
    const sc = new SplitContainer(leaf('t1'), {
      paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
    });

    mountWithStyles(sc);
    expect(document.body.querySelector('.split-root')).not.toBeNull();

    sc.destroy();
    expect(document.body.querySelector('.split-root')).toBeNull();
  });
});
