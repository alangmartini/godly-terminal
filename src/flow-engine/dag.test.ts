import { describe, it, expect } from 'vitest';
import {
  buildAdjacencyList,
  buildInDegreeMap,
  topologicalSort,
  findTriggerNodes,
  detectCycle,
} from './dag';
import type { FlowNode, FlowEdge } from './types';

function node(id: string, type = 'terminal.execute'): FlowNode {
  return { id, type, label: id, position: { x: 0, y: 0 }, config: {}, disabled: false };
}

function edge(sourceNodeId: string, targetNodeId: string): FlowEdge {
  return { id: `${sourceNodeId}->${targetNodeId}`, sourceNodeId, sourcePort: 'out', targetNodeId, targetPort: 'in' };
}

describe('buildAdjacencyList', () => {
  it('creates empty sets for nodes with no edges', () => {
    const adj = buildAdjacencyList([node('a'), node('b')], []);
    expect(adj.get('a')!.size).toBe(0);
    expect(adj.get('b')!.size).toBe(0);
  });

  it('maps source nodes to their targets', () => {
    const adj = buildAdjacencyList(
      [node('a'), node('b'), node('c')],
      [edge('a', 'b'), edge('a', 'c')],
    );
    expect([...adj.get('a')!]).toEqual(expect.arrayContaining(['b', 'c']));
    expect(adj.get('b')!.size).toBe(0);
  });
});

describe('buildInDegreeMap', () => {
  it('returns zero for nodes with no incoming edges', () => {
    const map = buildInDegreeMap([node('a'), node('b')], []);
    expect(map.get('a')).toBe(0);
    expect(map.get('b')).toBe(0);
  });

  it('counts incoming edges correctly', () => {
    const map = buildInDegreeMap(
      [node('a'), node('b'), node('c')],
      [edge('a', 'c'), edge('b', 'c')],
    );
    expect(map.get('a')).toBe(0);
    expect(map.get('b')).toBe(0);
    expect(map.get('c')).toBe(2);
  });
});

describe('topologicalSort', () => {
  it('sorts a linear chain into sequential layers', () => {
    const nodes = [node('a'), node('b'), node('c')];
    const edges = [edge('a', 'b'), edge('b', 'c')];
    const layers = topologicalSort(nodes, edges);
    expect(layers).toEqual([['a'], ['b'], ['c']]);
  });

  it('groups independent nodes into the same layer', () => {
    // a -> b, a -> c (b and c are in the same layer)
    const nodes = [node('a'), node('b'), node('c')];
    const edges = [edge('a', 'b'), edge('a', 'c')];
    const layers = topologicalSort(nodes, edges);
    expect(layers[0]).toEqual(['a']);
    expect(layers[1]).toEqual(expect.arrayContaining(['b', 'c']));
    expect(layers[1]).toHaveLength(2);
  });

  it('handles diamond DAG correctly', () => {
    // a -> b, a -> c, b -> d, c -> d
    const nodes = [node('a'), node('b'), node('c'), node('d')];
    const edges = [edge('a', 'b'), edge('a', 'c'), edge('b', 'd'), edge('c', 'd')];
    const layers = topologicalSort(nodes, edges);
    expect(layers).toHaveLength(3);
    expect(layers[0]).toEqual(['a']);
    expect(layers[1]).toEqual(expect.arrayContaining(['b', 'c']));
    expect(layers[2]).toEqual(['d']);
  });

  it('handles disconnected nodes', () => {
    const nodes = [node('a'), node('b')];
    const layers = topologicalSort(nodes, []);
    expect(layers).toHaveLength(1);
    expect(layers[0]).toEqual(expect.arrayContaining(['a', 'b']));
  });

  it('throws on cycle', () => {
    const nodes = [node('a'), node('b')];
    const edges = [edge('a', 'b'), edge('b', 'a')];
    expect(() => topologicalSort(nodes, edges)).toThrow(/Cycle detected/);
  });
});

describe('findTriggerNodes', () => {
  it('returns trigger-type nodes with no incoming edges', () => {
    const nodes = [node('t', 'trigger.hotkey'), node('a'), node('b')];
    const edges = [edge('t', 'a'), edge('a', 'b')];
    const triggers = findTriggerNodes(nodes, edges);
    expect(triggers).toHaveLength(1);
    expect(triggers[0].id).toBe('t');
  });

  it('excludes trigger nodes that have incoming edges', () => {
    const nodes = [node('a'), node('t', 'trigger.hotkey')];
    const edges = [edge('a', 't')];
    const triggers = findTriggerNodes(nodes, edges);
    expect(triggers).toHaveLength(0);
  });

  it('excludes non-trigger nodes', () => {
    const nodes = [node('a', 'terminal.create')];
    const triggers = findTriggerNodes(nodes, []);
    expect(triggers).toHaveLength(0);
  });
});

describe('detectCycle', () => {
  it('returns false for acyclic graph', () => {
    const nodes = [node('a'), node('b'), node('c')];
    const edges = [edge('a', 'b'), edge('b', 'c')];
    expect(detectCycle(nodes, edges)).toBe(false);
  });

  it('returns true for direct cycle', () => {
    const nodes = [node('a'), node('b')];
    const edges = [edge('a', 'b'), edge('b', 'a')];
    expect(detectCycle(nodes, edges)).toBe(true);
  });

  it('returns true for indirect cycle', () => {
    const nodes = [node('a'), node('b'), node('c')];
    const edges = [edge('a', 'b'), edge('b', 'c'), edge('c', 'a')];
    expect(detectCycle(nodes, edges)).toBe(true);
  });

  it('returns false for empty graph', () => {
    expect(detectCycle([], [])).toBe(false);
  });
});
