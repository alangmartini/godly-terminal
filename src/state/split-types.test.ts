import { describe, it, expect } from 'vitest';
import {
  LayoutNode,
  LeafNode,
  SplitNode,
  findTerminal,
  terminalIds,
  countLeaves,
  replaceLeaf,
  removeLeaf,
  removeTerminal,
  containsTerminal,
  getNodeAtPath,
  updateRatioAtPath,
  splitAt,
  swapTerminals,
  updateRatio,
  fromLegacySplitView,
  findAdjacentTerminal,
  replaceTerminal,
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
});
