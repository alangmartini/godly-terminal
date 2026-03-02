import { describe, it, expect } from 'vitest';
import {
  LayoutNode,
  LeafNode,
  SplitNode,
  GridNode,
  findTerminal,
  terminalIds,
  countLeaves,
  replaceLeaf,
  removeLeaf,
  removeTerminal,
  containsTerminal,
  getNodeAtPath,
  updateRatioAtPath,
  updateGridRatioAtPath,
  splitAt,
  swapTerminals,
  updateRatio,
  fromLegacySplitView,
  findAdjacentTerminal,
  replaceTerminal,
  maybePromoteToGrid,
} from './split-types';

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

describe('split-types utilities', () => {

  describe('findTerminal', () => {
    it('finds terminal in a single leaf', () => {
      expect(findTerminal(leaf('t1'), 't1')).toBe(true);
    });

    it('returns false for missing terminal in a leaf', () => {
      expect(findTerminal(leaf('t1'), 't2')).toBe(false);
    });

    it('finds terminal in left child of split', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(findTerminal(tree, 't1')).toBe(true);
    });

    it('finds terminal in right child of split', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(findTerminal(tree, 't2')).toBe(true);
    });

    it('finds terminal in deeply nested tree', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      expect(findTerminal(tree, 't3')).toBe(true);
    });

    it('returns false for non-existent terminal in nested tree', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      expect(findTerminal(tree, 't4')).toBe(false);
    });
  });

  describe('containsTerminal', () => {
    it('should find terminal in leaf', () => {
      expect(containsTerminal(leaf('t1'), 't1')).toBe(true);
      expect(containsTerminal(leaf('t1'), 't2')).toBe(false);
    });

    it('should find terminal in nested tree', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      expect(containsTerminal(tree, 't3')).toBe(true);
      expect(containsTerminal(tree, 't4')).toBe(false);
    });
  });

  describe('terminalIds', () => {
    it('returns single id for leaf', () => {
      expect(terminalIds(leaf('t1'))).toEqual(['t1']);
    });

    it('returns left-to-right order for split', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(terminalIds(tree)).toEqual(['t1', 't2']);
    });

    it('returns depth-first left-to-right for nested splits', () => {
      const tree = split('vertical',
        split('horizontal', leaf('t1'), leaf('t2')),
        split('horizontal', leaf('t3'), leaf('t4')),
      );
      expect(terminalIds(tree)).toEqual(['t1', 't2', 't3', 't4']);
    });

    it('should handle deeply nested tree', () => {
      const tree = split('horizontal',
        split('vertical', leaf('t1'), leaf('t2')),
        split('vertical', leaf('t3'), leaf('t4')),
      );
      expect(terminalIds(tree)).toEqual(['t1', 't2', 't3', 't4']);
    });
  });

  describe('countLeaves', () => {
    it('returns 1 for a leaf', () => {
      expect(countLeaves(leaf('t1'))).toBe(1);
    });

    it('returns 2 for a simple split', () => {
      expect(countLeaves(split('horizontal', leaf('t1'), leaf('t2')))).toBe(2);
    });

    it('returns 4 for a 2x2 grid', () => {
      const tree = split('vertical',
        split('horizontal', leaf('t1'), leaf('t2')),
        split('horizontal', leaf('t3'), leaf('t4')),
      );
      expect(countLeaves(tree)).toBe(4);
    });
  });

  describe('replaceLeaf', () => {
    it('should replace a leaf at root', () => {
      const result = replaceLeaf(leaf('t1'), 't1', leaf('t2'));
      expect(result).toEqual(leaf('t2'));
    });

    it('should return null if leaf not found', () => {
      const result = replaceLeaf(leaf('t1'), 't3', leaf('t2'));
      expect(result).toBeNull();
    });

    it('should replace a leaf in nested tree', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      const replacement = split('horizontal', leaf('t2'), leaf('t4'));
      const result = replaceLeaf(tree, 't2', replacement);

      expect(result).not.toBeNull();
      expect(terminalIds(result!)).toEqual(['t1', 't2', 't4', 't3']);
    });
  });

  describe('removeLeaf', () => {
    it('should remove leaf from 2-pane split, returning sibling', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      const { result, found } = removeLeaf(tree, 't1');
      expect(found).toBe(true);
      expect(result).toEqual(leaf('t2'));
    });

    it('should remove leaf from nested tree, collapsing parent split', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      const { result, found } = removeLeaf(tree, 't3');
      expect(found).toBe(true);
      expect(result).toEqual(split('horizontal', leaf('t1'), leaf('t2')));
    });

    it('should not find non-existent terminal', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      const { result, found } = removeLeaf(tree, 't3');
      expect(found).toBe(false);
      expect(result).toEqual(tree);
    });

    it('should return null when removing only terminal', () => {
      const { result, found } = removeLeaf(leaf('t1'), 't1');
      expect(found).toBe(true);
      expect(result).toBeNull();
    });
  });

  describe('removeTerminal', () => {
    it('returns null when removing the only leaf', () => {
      expect(removeTerminal(leaf('t1'), 't1')).toBeNull();
    });

    it('returns unchanged leaf when id does not match', () => {
      const node = leaf('t1');
      expect(removeTerminal(node, 't2')).toBe(node);
    });

    it('collapses split when removing left child', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      const result = removeTerminal(tree, 't1');
      expect(result).toEqual(leaf('t2'));
    });

    it('collapses split when removing right child', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      const result = removeTerminal(tree, 't2');
      expect(result).toEqual(leaf('t1'));
    });

    it('collapses nested split when removing deeply nested leaf', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      const result = removeTerminal(tree, 't2');
      expect(result).toEqual(split('horizontal', leaf('t1'), leaf('t3')));
    });

    it('collapses to sibling subtree when removing from 2x2 grid', () => {
      const tree = split('vertical',
        split('horizontal', leaf('t1'), leaf('t2')),
        split('horizontal', leaf('t3'), leaf('t4')),
      );
      const result = removeTerminal(tree, 't1');
      expect(result).toEqual(split('vertical',
        leaf('t2'),
        split('horizontal', leaf('t3'), leaf('t4')),
      ));
    });

    it('returns unchanged tree when removing non-existent id', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(removeTerminal(tree, 't3')).toBe(tree);
    });

    it('handles removing from 3-pane nested split correctly', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('horizontal', leaf('t2'), leaf('t3')),
      );
      const result = removeTerminal(tree, 't3');
      expect(result).toEqual(split('horizontal', leaf('t1'), leaf('t2')));
    });
  });

  describe('getNodeAtPath', () => {
    it('should return root at empty path', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(getNodeAtPath(tree, [])).toEqual(tree);
    });

    it('should return first child at [0]', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(getNodeAtPath(tree, [0])).toEqual(leaf('t1'));
    });

    it('should return second child at [1]', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(getNodeAtPath(tree, [1])).toEqual(leaf('t2'));
    });

    it('should navigate nested path', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3'), 0.7),
      );
      const node = getNodeAtPath(tree, [1, 0]);
      expect(node).toEqual(leaf('t2'));
    });

    it('should return null for invalid path', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(getNodeAtPath(tree, [0, 0])).toBeNull();
      expect(getNodeAtPath(tree, [2])).toBeNull();
    });
  });

  describe('updateRatioAtPath', () => {
    it('should update root ratio', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      const result = updateRatioAtPath(tree, [], 0.7);
      expect(result).not.toBeNull();
      if (result!.type === 'split') {
        expect(result!.ratio).toBe(0.7);
      }
    });

    it('should update nested ratio', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      const result = updateRatioAtPath(tree, [1], 0.3);
      expect(result).not.toBeNull();
      if (result!.type === 'split' && result!.second.type === 'split') {
        expect(result!.second.ratio).toBe(0.3);
        expect(result!.ratio).toBe(0.5); // root unchanged
      }
    });

    it('should return null for path pointing to leaf', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(updateRatioAtPath(tree, [0], 0.3)).toBeNull();
    });
  });

  describe('splitAt', () => {
    it('splits a leaf node into a two-pane split', () => {
      const result = splitAt(leaf('t1'), 't1', 't2', 'horizontal');
      expect(result).toEqual(split('horizontal', leaf('t1'), leaf('t2')));
    });

    it('respects custom ratio', () => {
      const result = splitAt(leaf('t1'), 't1', 't2', 'vertical', 0.7);
      expect(result).toEqual({
        type: 'split',
        direction: 'vertical',
        ratio: 0.7,
        first: leaf('t1'),
        second: leaf('t2'),
      });
    });

    it('splits a leaf within a nested tree', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      const result = splitAt(tree, 't2', 't3', 'vertical');
      expect(result).toEqual(split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      ));
    });

    it('returns unchanged tree when targetId not found', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(splitAt(tree, 't3', 't4', 'horizontal')).toBe(tree);
    });

    it('creates 4-pane grid from 2-pane split', () => {
      let tree: LayoutNode = split('horizontal', leaf('t1'), leaf('t2'));
      tree = splitAt(tree, 't1', 't3', 'vertical');
      tree = splitAt(tree, 't2', 't4', 'vertical');
      expect(countLeaves(tree)).toBe(4);
      expect(terminalIds(tree)).toEqual(['t1', 't3', 't2', 't4']);
    });
  });

  describe('swapTerminals', () => {
    it('should swap two terminals', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      const result = swapTerminals(tree, 't1', 't2');
      expect(result).not.toBeNull();
      expect(terminalIds(result!)).toEqual(['t2', 't1']);
    });

    it('should swap in nested tree', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      const result = swapTerminals(tree, 't1', 't3');
      expect(result).not.toBeNull();
      expect(terminalIds(result!)).toEqual(['t3', 't2', 't1']);
    });

    it('should return null if terminal not found', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(swapTerminals(tree, 't1', 't4')).toBeNull();
    });

    it('handles single leaf swap (no-op if same id)', () => {
      const node = leaf('t1');
      expect(swapTerminals(node, 't1', 't1')).toEqual(leaf('t1'));
    });
  });

  describe('updateRatio', () => {
    it('updates ratio of matching split', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'), 0.5);
      const result = updateRatio(tree, 't1', 'horizontal', 0.1);
      expect((result as SplitNode).ratio).toBeCloseTo(0.6);
    });

    it('clamps ratio to minimum 0.15', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'), 0.2);
      const result = updateRatio(tree, 't1', 'horizontal', -0.1);
      expect((result as SplitNode).ratio).toBeCloseTo(0.15);
    });

    it('clamps ratio to maximum 0.85', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'), 0.8);
      const result = updateRatio(tree, 't1', 'horizontal', 0.1);
      expect((result as SplitNode).ratio).toBeCloseTo(0.85);
    });

    it('only updates split matching direction', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3'), 0.5),
      );
      const result = updateRatio(tree, 't2', 'vertical', 0.1);
      const inner = (result as SplitNode).second as SplitNode;
      expect(inner.ratio).toBeCloseTo(0.6);
      expect((result as SplitNode).ratio).toBe(0.5);
    });

    it('returns unchanged tree when target not found', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(updateRatio(tree, 't3', 'horizontal', 0.1)).toBe(tree);
    });
  });

  describe('fromLegacySplitView', () => {
    it('converts horizontal split', () => {
      const result = fromLegacySplitView({
        leftTerminalId: 't1',
        rightTerminalId: 't2',
        direction: 'horizontal',
        ratio: 0.5,
      });
      expect(result).toEqual(split('horizontal', leaf('t1'), leaf('t2')));
    });

    it('converts vertical split', () => {
      const result = fromLegacySplitView({
        leftTerminalId: 't1',
        rightTerminalId: 't2',
        direction: 'vertical',
        ratio: 0.7,
      });
      expect(result).toEqual({
        type: 'split',
        direction: 'vertical',
        ratio: 0.7,
        first: leaf('t1'),
        second: leaf('t2'),
      });
    });

    it('defaults unknown direction to horizontal', () => {
      const result = fromLegacySplitView({
        leftTerminalId: 't1',
        rightTerminalId: 't2',
        direction: 'something_else',
        ratio: 0.5,
      });
      expect((result as SplitNode).direction).toBe('horizontal');
    });
  });

  describe('findAdjacentTerminal', () => {
    it('should find right neighbor in horizontal split', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(findAdjacentTerminal(tree, 't1', 'horizontal', true)).toBe('t2');
    });

    it('should find left neighbor in horizontal split', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(findAdjacentTerminal(tree, 't2', 'horizontal', false)).toBe('t1');
    });

    it('should return null for wrong direction', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(findAdjacentTerminal(tree, 't1', 'vertical', true)).toBeNull();
    });

    it('should navigate across nested splits', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      expect(findAdjacentTerminal(tree, 't1', 'horizontal', true)).toBe('t2');
      expect(findAdjacentTerminal(tree, 't3', 'horizontal', false)).toBe('t1');
    });

    it('should return null for terminal not in tree', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(findAdjacentTerminal(tree, 't3', 'horizontal', true)).toBeNull();
    });

    it('should return null for leaf node', () => {
      expect(findAdjacentTerminal(leaf('t1'), 't1', 'horizontal', true)).toBeNull();
    });
  });

  describe('replaceTerminal', () => {
    it('replaces terminal in a leaf', () => {
      expect(replaceTerminal(leaf('t1'), 't1', 't5')).toEqual(leaf('t5'));
    });

    it('returns unchanged leaf when id does not match', () => {
      const node = leaf('t1');
      expect(replaceTerminal(node, 't2', 't5')).toBe(node);
    });

    it('replaces terminal in nested tree', () => {
      const tree = split('horizontal',
        leaf('t1'),
        split('vertical', leaf('t2'), leaf('t3')),
      );
      const result = replaceTerminal(tree, 't3', 't5');
      expect(terminalIds(result)).toEqual(['t1', 't2', 't5']);
    });
  });

  // -------------------------------------------------------------------
  // GridNode tests
  // -------------------------------------------------------------------

  function gridNode(
    tl: LayoutNode, tr: LayoutNode, bl: LayoutNode, br: LayoutNode,
    colRatios: [number, number] = [0.5, 0.5],
    rowRatios: [number, number] = [0.5, 0.5],
  ): GridNode {
    return { type: 'grid', colRatios, rowRatios, children: [tl, tr, bl, br] };
  }

  describe('maybePromoteToGrid', () => {
    it('promotes H(V(leaf,leaf), V(leaf,leaf)) to grid', () => {
      const tree = split('horizontal',
        split('vertical', leaf('tl'), leaf('bl'), 0.6),
        split('vertical', leaf('tr'), leaf('br'), 0.4),
        0.5,
      );
      const result = maybePromoteToGrid(tree);
      expect(result.type).toBe('grid');
      if (result.type === 'grid') {
        expect(terminalIds(result)).toEqual(['tl', 'tr', 'bl', 'br']);
        expect(result.colRatios).toEqual([0.5, 0.5]);
        expect(result.rowRatios).toEqual([0.6, 0.4]);
      }
    });

    it('promotes V(H(leaf,leaf), H(leaf,leaf)) to grid', () => {
      const tree = split('vertical',
        split('horizontal', leaf('tl'), leaf('tr'), 0.6),
        split('horizontal', leaf('bl'), leaf('br'), 0.4),
        0.5,
      );
      const result = maybePromoteToGrid(tree);
      expect(result.type).toBe('grid');
      if (result.type === 'grid') {
        expect(terminalIds(result)).toEqual(['tl', 'tr', 'bl', 'br']);
        expect(result.colRatios).toEqual([0.6, 0.4]);
        expect(result.rowRatios).toEqual([0.5, 0.5]);
      }
    });

    it('does not promote non-2x2 patterns', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(maybePromoteToGrid(tree).type).toBe('split');
    });

    it('does not promote when children have non-leaf grandchildren', () => {
      const tree = split('horizontal',
        split('vertical', leaf('t1'), split('horizontal', leaf('t2'), leaf('t3'))),
        split('vertical', leaf('t4'), leaf('t5')),
      );
      expect(maybePromoteToGrid(tree).type).toBe('split');
    });

    it('returns leaf unchanged', () => {
      expect(maybePromoteToGrid(leaf('t1')).type).toBe('leaf');
    });

    it('returns grid unchanged', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      expect(maybePromoteToGrid(g)).toBe(g);
    });

    it('promotes via real split flow (split right, then split each down)', () => {
      let tree: LayoutNode = leaf('t1');
      tree = splitAt(tree, 't1', 't2', 'horizontal');
      tree = splitAt(tree, 't1', 't3', 'vertical');
      tree = splitAt(tree, 't2', 't4', 'vertical');
      const result = maybePromoteToGrid(tree);
      expect(result.type).toBe('grid');
      expect(terminalIds(result)).toEqual(['t1', 't2', 't3', 't4']);
    });
  });

  describe('grid - findTerminal', () => {
    const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));

    it('finds all 4 terminals', () => {
      expect(findTerminal(g, 't1')).toBe(true);
      expect(findTerminal(g, 't2')).toBe(true);
      expect(findTerminal(g, 't3')).toBe(true);
      expect(findTerminal(g, 't4')).toBe(true);
    });

    it('returns false for missing terminal', () => {
      expect(findTerminal(g, 't5')).toBe(false);
    });
  });

  describe('grid - terminalIds', () => {
    it('returns TL, TR, BL, BR order', () => {
      const g = gridNode(leaf('tl'), leaf('tr'), leaf('bl'), leaf('br'));
      expect(terminalIds(g)).toEqual(['tl', 'tr', 'bl', 'br']);
    });
  });

  describe('grid - countLeaves', () => {
    it('returns 4', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      expect(countLeaves(g)).toBe(4);
    });
  });

  describe('grid - removeLeaf', () => {
    it('removes TL → collapses to 3-pane split', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      const { result, found } = removeLeaf(g, 't1');
      expect(found).toBe(true);
      expect(result).not.toBeNull();
      expect(countLeaves(result!)).toBe(3);
      expect(findTerminal(result!, 't2')).toBe(true);
      expect(findTerminal(result!, 't3')).toBe(true);
      expect(findTerminal(result!, 't4')).toBe(true);
    });

    it('removes BR → collapses to 3-pane split', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      const { result, found } = removeLeaf(g, 't4');
      expect(found).toBe(true);
      expect(countLeaves(result!)).toBe(3);
      expect(findTerminal(result!, 't1')).toBe(true);
      expect(findTerminal(result!, 't2')).toBe(true);
      expect(findTerminal(result!, 't3')).toBe(true);
    });

    it('returns not found for missing terminal', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      const { found } = removeLeaf(g, 't5');
      expect(found).toBe(false);
    });
  });

  describe('grid - removeTerminal', () => {
    it('removes from grid and collapses', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      const result = removeTerminal(g, 't2');
      expect(result).not.toBeNull();
      expect(countLeaves(result!)).toBe(3);
    });

    it('returns unchanged grid for missing id', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      expect(removeTerminal(g, 't5')).toBe(g);
    });
  });

  describe('grid - getNodeAtPath', () => {
    it('returns grid at empty path', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      expect(getNodeAtPath(g, [])).toBe(g);
    });

    it('returns children at indices 0-3', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      expect(getNodeAtPath(g, [0])).toEqual(leaf('t1'));
      expect(getNodeAtPath(g, [1])).toEqual(leaf('t2'));
      expect(getNodeAtPath(g, [2])).toEqual(leaf('t3'));
      expect(getNodeAtPath(g, [3])).toEqual(leaf('t4'));
    });

    it('returns null for invalid index', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      expect(getNodeAtPath(g, [4])).toBeNull();
    });
  });

  describe('grid - updateGridRatioAtPath', () => {
    it('updates col0 ratio at root grid', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      const result = updateGridRatioAtPath(g, [], 'col0', 0.7);
      expect(result).not.toBeNull();
      if (result?.type === 'grid') {
        expect(result.colRatios[0]).toBe(0.7);
        expect(result.colRatios[1]).toBe(0.5);
      }
    });

    it('updates row1 ratio at root grid', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      const result = updateGridRatioAtPath(g, [], 'row1', 0.3);
      expect(result).not.toBeNull();
      if (result?.type === 'grid') {
        expect(result.rowRatios[0]).toBe(0.5);
        expect(result.rowRatios[1]).toBe(0.3);
      }
    });

    it('returns null when path points to non-grid', () => {
      const tree = split('horizontal', leaf('t1'), leaf('t2'));
      expect(updateGridRatioAtPath(tree, [], 'col0', 0.7)).toBeNull();
    });
  });

  describe('grid - findAdjacentTerminal', () => {
    const g = gridNode(leaf('tl'), leaf('tr'), leaf('bl'), leaf('br'));

    it('TL right → TR', () => {
      expect(findAdjacentTerminal(g, 'tl', 'horizontal', true)).toBe('tr');
    });

    it('TR left → TL', () => {
      expect(findAdjacentTerminal(g, 'tr', 'horizontal', false)).toBe('tl');
    });

    it('TL down → BL', () => {
      expect(findAdjacentTerminal(g, 'tl', 'vertical', true)).toBe('bl');
    });

    it('BL up → TL', () => {
      expect(findAdjacentTerminal(g, 'bl', 'vertical', false)).toBe('tl');
    });

    it('BR left → BL', () => {
      expect(findAdjacentTerminal(g, 'br', 'horizontal', false)).toBe('bl');
    });

    it('TR down → BR', () => {
      expect(findAdjacentTerminal(g, 'tr', 'vertical', true)).toBe('br');
    });

    it('TL left → null (edge)', () => {
      expect(findAdjacentTerminal(g, 'tl', 'horizontal', false)).toBeNull();
    });

    it('BR down → null (edge)', () => {
      expect(findAdjacentTerminal(g, 'br', 'vertical', true)).toBeNull();
    });
  });

  describe('grid - swapTerminals', () => {
    it('swaps TL and BR', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      const result = swapTerminals(g, 't1', 't4');
      expect(result).not.toBeNull();
      expect(terminalIds(result!)).toEqual(['t4', 't2', 't3', 't1']);
    });
  });

  describe('grid - replaceTerminal', () => {
    it('replaces a terminal in the grid', () => {
      const g = gridNode(leaf('t1'), leaf('t2'), leaf('t3'), leaf('t4'));
      const result = replaceTerminal(g, 't3', 't5');
      expect(terminalIds(result)).toEqual(['t1', 't2', 't5', 't4']);
    });
  });

  describe('grid - updateRatio (keyboard resize)', () => {
    it('updates col ratio for top-row terminal', () => {
      const g = gridNode(leaf('tl'), leaf('tr'), leaf('bl'), leaf('br'));
      const result = updateRatio(g, 'tl', 'horizontal', 0.1);
      if (result.type === 'grid') {
        expect(result.colRatios[0]).toBeCloseTo(0.6);
        expect(result.colRatios[1]).toBe(0.5); // bottom row unchanged
      }
    });

    it('updates row ratio for left-col terminal', () => {
      const g = gridNode(leaf('tl'), leaf('tr'), leaf('bl'), leaf('br'));
      const result = updateRatio(g, 'tl', 'vertical', 0.1);
      if (result.type === 'grid') {
        expect(result.rowRatios[0]).toBeCloseTo(0.6);
        expect(result.rowRatios[1]).toBe(0.5); // right col unchanged
      }
    });
  });
});
