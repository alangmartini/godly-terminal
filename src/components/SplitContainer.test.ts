// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach } from 'vitest';
import { SplitContainer, SplitPaneHandle } from './SplitContainer';
import { LayoutNode, LeafNode, SplitNode, fromLegacySplitView } from '../state/split-types';

// Helpers
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

/** Create a mock pane handle with a real DOM container. */
function mockPane(id: string): SplitPaneHandle {
  const container = document.createElement('div');
  container.className = 'terminal-pane';
  container.dataset.id = id;
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

describe('SplitContainer', () => {
  let onRatioChange: ReturnType<typeof vi.fn>;
  let onFocusPane: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onRatioChange = vi.fn();
    onFocusPane = vi.fn();
  });

  describe('rendering a single leaf', () => {
    it('returns the pane container element directly', () => {
      const paneMap = createPaneMap(['t1']);
      const sc = new SplitContainer(leaf('t1'), {
        paneMap,
        onRatioChange,
        onFocusPane,
        focusedTerminalId: 't1',
      });

      const el = sc.getElement();
      expect(el.className).toBe('split-root');
      // The root should contain the pane container
      expect(el.children.length).toBe(1);
      expect((el.children[0] as HTMLElement).dataset.id).toBe('t1');
    });

    it('sets split-visible and split-focused on the pane', () => {
      const paneMap = createPaneMap(['t1']);
      new SplitContainer(leaf('t1'), {
        paneMap,
        onRatioChange,
        onFocusPane,
        focusedTerminalId: 't1',
      });

      expect(paneMap.get('t1')!.setSplitVisible).toHaveBeenCalledWith(true, true);
    });

    it('creates placeholder for missing pane', () => {
      const paneMap = createPaneMap([]);
      const sc = new SplitContainer(leaf('t1'), {
        paneMap,
        onRatioChange,
        onFocusPane,
        focusedTerminalId: 't1',
      });

      const el = sc.getElement();
      const placeholder = el.querySelector('.split-pane-placeholder');
      expect(placeholder).not.toBeNull();
      expect((placeholder as HTMLElement).dataset.terminalId).toBe('t1');
    });
  });

  describe('rendering a 2-pane horizontal split', () => {
    it('creates a flex row container with divider', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(
        split('horizontal', leaf('t1'), leaf('t2')),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      const el = sc.getElement();
      const container = el.querySelector('.split-container.horizontal');
      expect(container).not.toBeNull();
      // Should have 3 children: first pane, divider, second pane
      expect(container!.children.length).toBe(3);
      expect(container!.children[1].className).toBe('split-divider horizontal');
    });

    it('sets flex-basis on both panes based on ratio', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(
        split('horizontal', leaf('t1'), leaf('t2'), 0.6),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      const container = sc.getElement().querySelector('.split-container')!;
      const firstPane = container.children[0] as HTMLElement;
      const secondPane = container.children[2] as HTMLElement;
      expect(firstPane.style.flexBasis).toBe('calc(60% - 1px)');
      expect(secondPane.style.flexBasis).toBe('calc(40% - 1px)');
    });
  });

  describe('rendering a 2-pane vertical split', () => {
    it('creates a flex column container', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(
        split('vertical', leaf('t1'), leaf('t2')),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      const el = sc.getElement();
      const container = el.querySelector('.split-container.vertical');
      expect(container).not.toBeNull();
      expect(container!.children[1].className).toBe('split-divider vertical');
    });
  });

  describe('rendering a 3-pane nested split', () => {
    it('creates nested flex containers', () => {
      // t1 | (t2 / t3)
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      const paneMap = createPaneMap(['t1', 't2', 't3']);
      const sc = new SplitContainer(tree, {
        paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
      });

      const el = sc.getElement();
      const outerContainer = el.querySelector('.split-container.horizontal');
      expect(outerContainer).not.toBeNull();
      // Outer: t1 pane, divider, inner split-container
      expect(outerContainer!.children.length).toBe(3);

      const innerContainer = outerContainer!.children[2] as HTMLElement;
      expect(innerContainer.className).toContain('split-container');
      expect(innerContainer.className).toContain('vertical');
      // Inner: t2, divider, t3
      expect(innerContainer.children.length).toBe(3);
    });

    it('marks all 3 panes as split-visible', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      const paneMap = createPaneMap(['t1', 't2', 't3']);
      new SplitContainer(tree, {
        paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't2',
      });

      expect(paneMap.get('t1')!.setSplitVisible).toHaveBeenCalledWith(true, false);
      expect(paneMap.get('t2')!.setSplitVisible).toHaveBeenCalledWith(true, true);
      expect(paneMap.get('t3')!.setSplitVisible).toHaveBeenCalledWith(true, false);
    });
  });

  describe('rendering a 4-pane grid (2x2)', () => {
    it('creates a 2x2 grid with 4 panes and 3 dividers', () => {
      // (t1 | t2) / (t3 | t4)
      const tree = split('vertical',
        split('horizontal', leaf('t1'), leaf('t2')),
        split('horizontal', leaf('t3'), leaf('t4')),
      );
      const paneMap = createPaneMap(['t1', 't2', 't3', 't4']);
      const sc = new SplitContainer(tree, {
        paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
      });

      const el = sc.getElement();
      // Count all dividers in the tree
      const dividers = el.querySelectorAll('.split-divider');
      expect(dividers.length).toBe(3); // 1 vertical outer + 2 horizontal inner

      // Count all terminal panes
      const panes = el.querySelectorAll('.terminal-pane');
      expect(panes.length).toBe(4);
    });
  });

  describe('divider drag', () => {
    it('calls onRatioChange during mousedown + mousemove', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(
        split('horizontal', leaf('t1'), leaf('t2'), 0.5),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      const divider = sc.getElement().querySelector('.split-divider') as HTMLElement;
      expect(divider).not.toBeNull();

      // Mock the parent container's getBoundingClientRect
      const container = divider.parentElement!;
      vi.spyOn(container, 'getBoundingClientRect').mockReturnValue({
        left: 0, right: 800, top: 0, bottom: 600,
        width: 800, height: 600, x: 0, y: 0,
        toJSON: () => ({}),
      });

      // Start drag
      divider.dispatchEvent(new MouseEvent('mousedown', {
        clientX: 400, clientY: 300, bubbles: true,
      }));

      // Move mouse to 60% position
      document.dispatchEvent(new MouseEvent('mousemove', {
        clientX: 480, clientY: 300,
      }));

      expect(onRatioChange).toHaveBeenCalledWith([], 0.6);
    });

    it('clamps ratio to min 0.15', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(
        split('horizontal', leaf('t1'), leaf('t2'), 0.5),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      const divider = sc.getElement().querySelector('.split-divider') as HTMLElement;
      const container = divider.parentElement!;
      vi.spyOn(container, 'getBoundingClientRect').mockReturnValue({
        left: 0, right: 800, top: 0, bottom: 600,
        width: 800, height: 600, x: 0, y: 0,
        toJSON: () => ({}),
      });

      divider.dispatchEvent(new MouseEvent('mousedown', {
        clientX: 400, clientY: 300, bubbles: true,
      }));

      // Move to 5% — should clamp to 0.15
      document.dispatchEvent(new MouseEvent('mousemove', {
        clientX: 40, clientY: 300,
      }));

      expect(onRatioChange).toHaveBeenCalledWith([], 0.15);
    });
  });

  describe('update method', () => {
    it('re-renders when tree structure changes', () => {
      const paneMap = createPaneMap(['t1', 't2', 't3']);
      const sc = new SplitContainer(
        split('horizontal', leaf('t1'), leaf('t2')),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      // Update to a 3-pane layout
      sc.update(split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      ));

      const el = sc.getElement();
      const dividers = el.querySelectorAll('.split-divider');
      expect(dividers.length).toBe(2);
    });

    it('skips re-render when tree is structurally identical', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const node = split('horizontal', leaf('t1'), leaf('t2'));
      const sc = new SplitContainer(node, {
        paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
      });

      const prevInner = sc.getElement().innerHTML;

      // Same structure, same ratios — no re-render needed
      sc.update(split('horizontal', leaf('t1'), leaf('t2')));
      expect(sc.getElement().innerHTML).toBe(prevInner);
    });

    it('re-renders when ratio changes', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(
        split('horizontal', leaf('t1'), leaf('t2'), 0.5),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      sc.update(split('horizontal', leaf('t1'), leaf('t2'), 0.7));

      const container = sc.getElement().querySelector('.split-container')!;
      const firstPane = container.children[0] as HTMLElement;
      expect(firstPane.style.flexBasis).toBe('calc(70% - 1px)');
    });
  });

  describe('updateFocus', () => {
    it('updates focus without changing structure', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(
        split('horizontal', leaf('t1'), leaf('t2')),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      vi.mocked(paneMap.get('t1')!.setSplitVisible).mockClear();
      vi.mocked(paneMap.get('t2')!.setSplitVisible).mockClear();

      sc.updateFocus('t2');

      expect(paneMap.get('t1')!.setSplitVisible).toHaveBeenCalledWith(true, false);
      expect(paneMap.get('t2')!.setSplitVisible).toHaveBeenCalledWith(true, true);
    });
  });

  describe('fromLegacySplitView integration', () => {
    it('renders legacy split as a 2-pane layout', () => {
      const tree = fromLegacySplitView({
        leftTerminalId: 't1',
        rightTerminalId: 't2',
        direction: 'horizontal',
        ratio: 0.5,
      });

      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(tree, {
        paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
      });

      const el = sc.getElement();
      expect(el.querySelectorAll('.split-divider').length).toBe(1);
      expect(el.querySelectorAll('.terminal-pane').length).toBe(2);
    });
  });

  describe('destroy', () => {
    it('removes the split-root from DOM', () => {
      const paneMap = createPaneMap(['t1']);
      const sc = new SplitContainer(leaf('t1'), {
        paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1',
      });

      const parent = document.createElement('div');
      parent.appendChild(sc.getElement());
      expect(parent.querySelector('.split-root')).not.toBeNull();

      sc.destroy();
      expect(parent.querySelector('.split-root')).toBeNull();
    });

    it('re-parents pane containers back to parent on destroy', () => {
      const paneMap = createPaneMap(['t1', 't2']);
      const sc = new SplitContainer(
        split('horizontal', leaf('t1'), leaf('t2')),
        { paneMap, onRatioChange, onFocusPane, focusedTerminalId: 't1' },
      );

      const parent = document.createElement('div');
      parent.appendChild(sc.getElement());

      sc.destroy();

      // Pane containers should be back in the parent, not orphaned
      expect(parent.contains(paneMap.get('t1')!.getContainer())).toBe(true);
      expect(parent.contains(paneMap.get('t2')!.getContainer())).toBe(true);
    });
  });
});
