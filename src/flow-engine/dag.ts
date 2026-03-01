import type { FlowNode, FlowEdge } from './types';

// ── Adjacency & In-Degree ────────────────────────────────────────────

/**
 * Build a forward adjacency list: for each node, the set of nodes it
 * points to (its direct dependents).
 */
export function buildAdjacencyList(
  nodes: FlowNode[],
  edges: FlowEdge[],
): Map<string, Set<string>> {
  const adj = new Map<string, Set<string>>();
  for (const node of nodes) {
    adj.set(node.id, new Set());
  }
  for (const edge of edges) {
    const targets = adj.get(edge.sourceNodeId);
    if (targets) {
      targets.add(edge.targetNodeId);
    }
  }
  return adj;
}

/**
 * Build an in-degree map: for each node, the count of incoming edges.
 */
export function buildInDegreeMap(
  nodes: FlowNode[],
  edges: FlowEdge[],
): Map<string, number> {
  const inDegree = new Map<string, number>();
  for (const node of nodes) {
    inDegree.set(node.id, 0);
  }
  for (const edge of edges) {
    const current = inDegree.get(edge.targetNodeId);
    if (current !== undefined) {
      inDegree.set(edge.targetNodeId, current + 1);
    }
  }
  return inDegree;
}

// ── Topological Sort (Kahn's Algorithm — layered) ────────────────────

/**
 * Perform a layered topological sort using Kahn's algorithm.
 *
 * Returns an array of layers, where each layer is an array of node IDs
 * whose dependencies are all satisfied by previous layers. Nodes within
 * the same layer can be executed concurrently.
 *
 * Throws if the graph contains a cycle.
 */
export function topologicalSort(
  nodes: FlowNode[],
  edges: FlowEdge[],
): string[][] {
  const adj = buildAdjacencyList(nodes, edges);
  const inDegree = buildInDegreeMap(nodes, edges);

  // Seed the first frontier with all zero-in-degree nodes
  let frontier: string[] = [];
  for (const [nodeId, degree] of inDegree) {
    if (degree === 0) {
      frontier.push(nodeId);
    }
  }

  const layers: string[][] = [];
  let visited = 0;

  while (frontier.length > 0) {
    layers.push([...frontier]);
    visited += frontier.length;

    const nextFrontier: string[] = [];
    for (const nodeId of frontier) {
      const targets = adj.get(nodeId);
      if (!targets) continue;
      for (const target of targets) {
        const newDegree = inDegree.get(target)! - 1;
        inDegree.set(target, newDegree);
        if (newDegree === 0) {
          nextFrontier.push(target);
        }
      }
    }
    frontier = nextFrontier;
  }

  if (visited !== nodes.length) {
    throw new Error(
      `Cycle detected in flow graph: sorted ${visited} of ${nodes.length} nodes`,
    );
  }

  return layers;
}

// ── Trigger Detection ────────────────────────────────────────────────

/**
 * Find all trigger nodes — nodes whose category is 'trigger' that have
 * no incoming edges.
 */
export function findTriggerNodes(
  nodes: FlowNode[],
  edges: FlowEdge[],
): FlowNode[] {
  const nodesWithIncoming = new Set<string>();
  for (const edge of edges) {
    nodesWithIncoming.add(edge.targetNodeId);
  }
  return nodes.filter(
    (node) => node.type.startsWith('trigger.') && !nodesWithIncoming.has(node.id),
  );
}

// ── Cycle Detection ──────────────────────────────────────────────────

/**
 * Detect whether the DAG contains a cycle using DFS with three-color
 * marking (white/gray/black).
 *
 * Returns true if a cycle exists.
 */
export function detectCycle(
  nodes: FlowNode[],
  edges: FlowEdge[],
): boolean {
  const adj = buildAdjacencyList(nodes, edges);

  const WHITE = 0;
  const GRAY = 1;
  const BLACK = 2;

  const color = new Map<string, number>();
  for (const node of nodes) {
    color.set(node.id, WHITE);
  }

  function dfs(nodeId: string): boolean {
    color.set(nodeId, GRAY);
    const neighbors = adj.get(nodeId);
    if (neighbors) {
      for (const neighbor of neighbors) {
        const c = color.get(neighbor);
        if (c === GRAY) return true; // back edge → cycle
        if (c === WHITE && dfs(neighbor)) return true;
      }
    }
    color.set(nodeId, BLACK);
    return false;
  }

  for (const node of nodes) {
    if (color.get(node.id) === WHITE) {
      if (dfs(node.id)) return true;
    }
  }

  return false;
}
