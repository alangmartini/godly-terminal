import { describe, it, expect, beforeEach, vi } from 'vitest';
import { FlowExecutor } from './flow-executor';
import { NodeTypeRegistry } from './node-type-registry';
import type { Flow, FlowNode, FlowEdge, NodeTypeDefinition } from './types';

function makeNode(id: string, type: string, config: Record<string, unknown> = {}): FlowNode {
  return { id, type, label: id, position: { x: 0, y: 0 }, config, disabled: false };
}

function makeEdge(srcId: string, srcPort: string, tgtId: string, tgtPort: string): FlowEdge {
  return { id: `${srcId}->${tgtId}`, sourceNodeId: srcId, sourcePort: srcPort, targetNodeId: tgtId, targetPort: tgtPort };
}

function makeFlow(nodes: FlowNode[], edges: FlowEdge[]): Flow {
  return {
    id: 'test-flow',
    name: 'Test',
    description: '',
    version: 1,
    schemaVersion: 1,
    tags: [],
    enabled: true,
    nodes,
    edges,
    variables: [],
    createdAt: Date.now(),
    updatedAt: Date.now(),
  };
}

function makeNodeDef(type: string, executeFn: NodeTypeDefinition['execute']): NodeTypeDefinition {
  return {
    type,
    category: 'terminal',
    label: type,
    description: '',
    icon: '',
    ports: [],
    configSchema: [],
    execute: executeFn,
  };
}

describe('FlowExecutor', () => {
  let registry: NodeTypeRegistry;
  let executor: FlowExecutor;

  beforeEach(() => {
    registry = new NodeTypeRegistry();
    executor = new FlowExecutor(registry);
  });

  it('executes a single-node flow', async () => {
    const fn = vi.fn().mockResolvedValue({ result: 'ok' });
    registry.register(makeNodeDef('test.action', fn));

    const flow = makeFlow(
      [makeNode('n1', 'test.action')],
      [],
    );

    const run = await executor.startRun(flow);
    expect(run.status).toBe('completed');
    expect(fn).toHaveBeenCalledTimes(1);

    const state = run.nodeStates.get('n1');
    expect(state?.status).toBe('completed');
    expect(state?.outputs).toEqual({ result: 'ok' });
  });

  it('executes a linear chain passing outputs through edges', async () => {
    registry.register(makeNodeDef('step.produce', async () => ({ value: 42 })));
    registry.register(makeNodeDef('step.consume', async (inputs) => ({ doubled: (inputs.value as number) * 2 })));

    const flow = makeFlow(
      [makeNode('n1', 'step.produce'), makeNode('n2', 'step.consume')],
      [makeEdge('n1', 'value', 'n2', 'value')],
    );

    const run = await executor.startRun(flow);
    expect(run.status).toBe('completed');

    const n2State = run.nodeStates.get('n2');
    expect(n2State?.outputs).toEqual({ doubled: 84 });
  });

  it('marks remaining nodes as skipped when a node fails', async () => {
    registry.register(makeNodeDef('step.ok', async () => ({})));
    registry.register(makeNodeDef('step.fail', async () => { throw new Error('boom'); }));
    registry.register(makeNodeDef('step.after', async () => ({ reached: true })));

    const flow = makeFlow(
      [makeNode('n1', 'step.ok'), makeNode('n2', 'step.fail'), makeNode('n3', 'step.after')],
      [makeEdge('n1', 'out', 'n2', 'in'), makeEdge('n2', 'out', 'n3', 'in')],
    );

    const run = await executor.startRun(flow);
    expect(run.status).toBe('failed');
    expect(run.nodeStates.get('n1')?.status).toBe('completed');
    expect(run.nodeStates.get('n2')?.status).toBe('failed');
    expect(run.nodeStates.get('n2')?.error).toBe('boom');
    expect(run.nodeStates.get('n3')?.status).toBe('skipped');
  });

  it('skips disabled nodes', async () => {
    const fn = vi.fn().mockResolvedValue({});
    registry.register(makeNodeDef('step.a', fn));

    const disabledNode = makeNode('n1', 'step.a');
    disabledNode.disabled = true;

    const flow = makeFlow([disabledNode], []);
    const run = await executor.startRun(flow);

    expect(run.status).toBe('completed');
    expect(fn).not.toHaveBeenCalled();
    expect(run.nodeStates.get('n1')?.status).toBe('skipped');
  });

  it('handles unknown node types gracefully', async () => {
    // Don't register any node type
    const flow = makeFlow([makeNode('n1', 'unknown.type')], []);
    const run = await executor.startRun(flow);

    expect(run.status).toBe('failed');
    expect(run.nodeStates.get('n1')?.status).toBe('failed');
    expect(run.nodeStates.get('n1')?.error).toContain('Unknown node type');
  });

  it('supports cancellation via cancelRun', async () => {
    let resolveNode: () => void;
    const hangingPromise = new Promise<Record<string, unknown>>((resolve) => {
      resolveNode = () => resolve({});
    });

    registry.register(makeNodeDef('step.hang', async (_inputs, _config, context) => {
      return new Promise((resolve, reject) => {
        const onAbort = () => reject(new Error('cancelled'));
        context.abortSignal.addEventListener('abort', onAbort, { once: true });
        hangingPromise.then(resolve);
      });
    }));

    const flow = makeFlow(
      [makeNode('n1', 'step.hang'), makeNode('n2', 'step.hang')],
      [makeEdge('n1', 'out', 'n2', 'in')],
    );

    const runPromise = executor.startRun(flow);

    // Give the executor a tick to start
    await new Promise((r) => setTimeout(r, 10));
    executor.cancelRun((await executor.getActiveRuns())[0]?.id ?? '');

    // Resolve the hanging promise so the test can complete
    resolveNode!();

    const run = await runPromise;
    // The run should be either cancelled or failed (depending on timing)
    expect(['cancelled', 'failed']).toContain(run.status);
  });

  it('notifies subscribers on state changes', async () => {
    const fn = vi.fn();
    registry.register(makeNodeDef('step.a', async () => ({})));

    executor.onRunUpdate(fn);

    const flow = makeFlow([makeNode('n1', 'step.a')], []);
    await executor.startRun(flow);

    // Should have been called at least: initial (running), node running, node completed, run completed
    expect(fn.mock.calls.length).toBeGreaterThanOrEqual(3);
  });

  it('passes trigger outputs to trigger nodes', async () => {
    let receivedInputs: Record<string, unknown> = {};
    registry.register({
      ...makeNodeDef('trigger.test', async (inputs) => {
        receivedInputs = inputs;
        return {};
      }),
      category: 'trigger',
    });

    const flow = makeFlow([makeNode('n1', 'trigger.test')], []);
    await executor.startRun(flow, { hotkey: 'Ctrl+T' });

    expect(receivedInputs).toEqual({ hotkey: 'Ctrl+T' });
  });

  it('executes parallel nodes in the same layer', async () => {
    const order: string[] = [];

    registry.register(makeNodeDef('step.root', async () => {
      order.push('root');
      return {};
    }));
    registry.register(makeNodeDef('step.left', async () => {
      order.push('left');
      return {};
    }));
    registry.register(makeNodeDef('step.right', async () => {
      order.push('right');
      return {};
    }));

    // root -> left, root -> right (left and right are in the same layer)
    const flow = makeFlow(
      [makeNode('root', 'step.root'), makeNode('left', 'step.left'), makeNode('right', 'step.right')],
      [makeEdge('root', 'out', 'left', 'in'), makeEdge('root', 'out', 'right', 'in')],
    );

    const run = await executor.startRun(flow);
    expect(run.status).toBe('completed');
    expect(order[0]).toBe('root');
    expect(order).toContain('left');
    expect(order).toContain('right');
  });
});
