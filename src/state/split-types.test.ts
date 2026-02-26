import { describe, it, expect } from 'vitest';
import {
  LayoutNode,
  terminalIds,
  replaceLeaf,
  removeLeaf,
  containsTerminal,
  updateRatioAtPath,
  swapTerminals,
  findAdjacentTerminal,
  getNodeAtPath,
} from './split-types';

const leaf = (id: string): LayoutNode => ({ type: 'leaf', terminal_id: id });
const split = (
  dir: 'horizontal' | 'vertical',
  ratio: number,
  first: LayoutNode,
  second: LayoutNode,
): LayoutNode => ({ type: 'split', direction: dir, ratio, first, second });

describe('split-types tree utilities', () => {
  describe('terminalIds', () => {
    it('should return single id for leaf', () => {
      expect(terminalIds(leaf('t1'))).toEqual(['t1']);
    });

    it('should return DFS order for nested tree', () => {
      const tree = split('horizontal', 0.5,
        leaf('t1'),
        split('vertical', 0.5, leaf('t2'), leaf('t3')),
      );
      expect(terminalIds(tree)).toEqual(['t1', 't2', 't3']);
    });

    it('should handle deeply nested tree', () => {
      const tree = split('horizontal', 0.5,
        split('vertical', 0.5, leaf('t1'), leaf('t2')),
        split('vertical', 0.5, leaf('t3'), leaf('t4')),
      );
      expect(terminalIds(tree)).toEqual(['t1', 't2', 't3', 't4']);
    });
  });

  describe('containsTerminal', () => {
    it('should find terminal in leaf', () => {
      expect(containsTerminal(leaf('t1'), 't1')).toBe(true);
      expect(containsTerminal(leaf('t1'), 't2')).toBe(false);
    });

    it('should find terminal in nested tree', () => {
      const tree = split('horizontal', 0.5,
        leaf('t1'),
        split('vertical', 0.5, leaf('t2'), leaf('t3')),
      );
      expect(containsTerminal(tree, 't3')).toBe(true);
      expect(containsTerminal(tree, 't4')).toBe(false);
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
      const tree = split('horizontal', 0.5,
        leaf('t1'),
        split('vertical', 0.5, leaf('t2'), leaf('t3')),
      );
      const replacement = split('horizontal', 0.5, leaf('t2'), leaf('t4'));
      const result = replaceLeaf(tree, 't2', replacement);

      expect(result).not.toBeNull();
      expect(terminalIds(result!)).toEqual(['t1', 't2', 't4', 't3']);
    });
  });

  describe('removeLeaf', () => {
    it('should remove leaf from 2-pane split, returning sibling', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      const { result, found } = removeLeaf(tree, 't1');
      expect(found).toBe(true);
      expect(result).toEqual(leaf('t2'));
    });

    it('should remove leaf from nested tree, collapsing parent split', () => {
      const tree = split('horizontal', 0.5,
        leaf('t1'),
        split('vertical', 0.5, leaf('t2'), leaf('t3')),
      );
      const { result, found } = removeLeaf(tree, 't3');
      expect(found).toBe(true);
      // Should collapse to: t1 | t2
      expect(result).toEqual(split('horizontal', 0.5, leaf('t1'), leaf('t2')));
    });

    it('should not find non-existent terminal', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
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

  describe('getNodeAtPath', () => {
    it('should return root at empty path', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(getNodeAtPath(tree, [])).toEqual(tree);
    });

    it('should return first child at [0]', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(getNodeAtPath(tree, [0])).toEqual(leaf('t1'));
    });

    it('should return second child at [1]', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(getNodeAtPath(tree, [1])).toEqual(leaf('t2'));
    });

    it('should navigate nested path', () => {
      const tree = split('horizontal', 0.5,
        leaf('t1'),
        split('vertical', 0.7, leaf('t2'), leaf('t3')),
      );
      const node = getNodeAtPath(tree, [1, 0]);
      expect(node).toEqual(leaf('t2'));
    });

    it('should return null for invalid path', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(getNodeAtPath(tree, [0, 0])).toBeNull(); // [0] is a leaf, can't go deeper
      expect(getNodeAtPath(tree, [2])).toBeNull(); // no third child
    });
  });

  describe('updateRatioAtPath', () => {
    it('should update root ratio', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      const result = updateRatioAtPath(tree, [], 0.7);
      expect(result).not.toBeNull();
      if (result!.type === 'split') {
        expect(result!.ratio).toBe(0.7);
      }
    });

    it('should update nested ratio', () => {
      const tree = split('horizontal', 0.5,
        leaf('t1'),
        split('vertical', 0.5, leaf('t2'), leaf('t3')),
      );
      const result = updateRatioAtPath(tree, [1], 0.3);
      expect(result).not.toBeNull();
      if (result!.type === 'split' && result!.second.type === 'split') {
        expect(result!.second.ratio).toBe(0.3);
        expect(result!.ratio).toBe(0.5); // root unchanged
      }
    });

    it('should return null for path pointing to leaf', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(updateRatioAtPath(tree, [0], 0.3)).toBeNull();
    });
  });

  describe('swapTerminals', () => {
    it('should swap two terminals', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      const result = swapTerminals(tree, 't1', 't2');
      expect(result).not.toBeNull();
      expect(terminalIds(result!)).toEqual(['t2', 't1']);
    });

    it('should swap in nested tree', () => {
      const tree = split('horizontal', 0.5,
        leaf('t1'),
        split('vertical', 0.5, leaf('t2'), leaf('t3')),
      );
      const result = swapTerminals(tree, 't1', 't3');
      expect(result).not.toBeNull();
      expect(terminalIds(result!)).toEqual(['t3', 't2', 't1']);
    });

    it('should return null if terminal not found', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(swapTerminals(tree, 't1', 't4')).toBeNull();
    });
  });

  describe('findAdjacentTerminal', () => {
    it('should find right neighbor in horizontal split', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(findAdjacentTerminal(tree, 't1', 'horizontal', true)).toBe('t2');
    });

    it('should find left neighbor in horizontal split', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(findAdjacentTerminal(tree, 't2', 'horizontal', false)).toBe('t1');
    });

    it('should return null for wrong direction', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(findAdjacentTerminal(tree, 't1', 'vertical', true)).toBeNull();
    });

    it('should navigate across nested splits', () => {
      // t1 | (t2 / t3)
      const tree = split('horizontal', 0.5,
        leaf('t1'),
        split('vertical', 0.5, leaf('t2'), leaf('t3')),
      );
      // From t1, going right should reach t2 (first leaf of right subtree)
      expect(findAdjacentTerminal(tree, 't1', 'horizontal', true)).toBe('t2');
      // From t3, going left should reach... nothing (t3 is in a vertical split)
      // The vertical split is the second child of horizontal. Going left from t3
      // means we need to find a horizontal split ancestor where t3 is in the second child.
      // t3 is in tree.second (horizontal split), so going left = tree.first = t1's last leaf = t1
      expect(findAdjacentTerminal(tree, 't3', 'horizontal', false)).toBe('t1');
    });

    it('should return null for terminal not in tree', () => {
      const tree = split('horizontal', 0.5, leaf('t1'), leaf('t2'));
      expect(findAdjacentTerminal(tree, 't3', 'horizontal', true)).toBeNull();
    });

    it('should return null for leaf node', () => {
      expect(findAdjacentTerminal(leaf('t1'), 't1', 'horizontal', true)).toBeNull();
    });
  });
});
