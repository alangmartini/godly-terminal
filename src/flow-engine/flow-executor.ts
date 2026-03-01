import type {
  Flow,
  FlowRun,
  FlowRunStatus,
  NodeRunState,
  NodeRunStatus,
  NodeExecutionContext,
} from './types';
import { topologicalSort } from './dag';
import type { NodeTypeRegistry } from './node-type-registry';

// ── Flow Executor ────────────────────────────────────────────────────

type RunSubscriber = (run: FlowRun) => void;

/**
 * DAG-walking executor for Godly Flows.
 *
 * Executes a flow layer-by-layer (as determined by topological sort).
 * Within each layer, nodes whose dependencies are all satisfied run
 * concurrently. For Phase 1 most flows are linear, but the executor
 * already supports fan-out parallelism within layers.
 *
 * Each run gets its own AbortController for cancellation. Node outputs
 * are threaded to downstream nodes by following edge connections.
 */
export class FlowExecutor {
  private activeRuns: Map<string, FlowRun> = new Map();
  private abortControllers: Map<string, AbortController> = new Map();
  private subscribers: Set<RunSubscriber> = new Set();

  constructor(private registry: NodeTypeRegistry) {}

  // ── Run Lifecycle ────────────────────────────────────────────────

  /**
   * Start executing a flow.
   *
   * @param flow          The flow definition to execute.
   * @param triggerOutputs Optional outputs to seed trigger nodes with
   *                       (e.g. the hotkey that triggered the flow).
   * @returns The completed FlowRun.
   */
  async startRun(
    flow: Flow,
    triggerOutputs?: Record<string, unknown>,
  ): Promise<FlowRun> {
    const runId = crypto.randomUUID();
    const abortController = new AbortController();

    // Build initial node states — skip disabled nodes upfront
    const nodeStates = new Map<string, NodeRunState>();
    for (const node of flow.nodes) {
      const status: NodeRunStatus = node.disabled ? 'skipped' : 'pending';
      nodeStates.set(node.id, {
        nodeId: node.id,
        status,
        outputs: {},
      });
    }

    const run: FlowRun = {
      id: runId,
      flowId: flow.id,
      status: 'running',
      nodeStates,
      startedAt: Date.now(),
    };

    this.activeRuns.set(runId, run);
    this.abortControllers.set(runId, abortController);
    this.notifySubscribers(run);

    try {
      // Filter to only enabled nodes for DAG processing
      const enabledNodes = flow.nodes.filter((n) => !n.disabled);
      const enabledEdges = flow.edges.filter(
        (e) =>
          enabledNodes.some((n) => n.id === e.sourceNodeId) &&
          enabledNodes.some((n) => n.id === e.targetNodeId),
      );

      const layers = topologicalSort(enabledNodes, enabledEdges);

      // Execute layer by layer
      for (const layer of layers) {
        if (abortController.signal.aborted) {
          this.markRemainingSkipped(run, nodeStates);
          this.finalizeRun(run, 'cancelled');
          return run;
        }

        // Execute all nodes in this layer concurrently
        const layerPromises = layer.map((nodeId) =>
          this.executeNode(nodeId, flow, run, nodeStates, triggerOutputs, abortController),
        );

        const results = await Promise.allSettled(layerPromises);

        // Check for failures — if any node in the layer failed, abort the run
        for (const result of results) {
          if (result.status === 'rejected') {
            // This shouldn't happen since executeNode catches internally,
            // but handle it defensively
            this.markRemainingSkipped(run, nodeStates);
            this.finalizeRun(run, 'failed', String(result.reason));
            return run;
          }
        }

        // Check if any node entered failed state
        const hasFailure = layer.some(
          (nodeId) => nodeStates.get(nodeId)?.status === 'failed',
        );
        if (hasFailure) {
          this.markRemainingSkipped(run, nodeStates);
          const failedNode = layer.find(
            (nodeId) => nodeStates.get(nodeId)?.status === 'failed',
          );
          const failedState = failedNode ? nodeStates.get(failedNode) : undefined;
          this.finalizeRun(run, 'failed', failedState?.error);
          return run;
        }
      }

      this.finalizeRun(run, 'completed');
    } catch (err) {
      this.markRemainingSkipped(run, nodeStates);
      this.finalizeRun(run, 'failed', err instanceof Error ? err.message : String(err));
    }

    return run;
  }

  /** Cancel an active run via its AbortController. */
  cancelRun(runId: string): void {
    const controller = this.abortControllers.get(runId);
    if (controller) {
      controller.abort();
    }
  }

  /** Return all currently active (running) runs. */
  getActiveRuns(): FlowRun[] {
    return Array.from(this.activeRuns.values()).filter(
      (run) => run.status === 'running',
    );
  }

  /** Look up a run by ID (active or recently completed). */
  getRun(runId: string): FlowRun | undefined {
    return this.activeRuns.get(runId);
  }

  // ── Subscriptions ────────────────────────────────────────────────

  /**
   * Subscribe to run state changes (for UI visualization).
   * Returns an unsubscribe function.
   */
  onRunUpdate(fn: RunSubscriber): () => void {
    this.subscribers.add(fn);
    return () => {
      this.subscribers.delete(fn);
    };
  }

  // ── Internal: Node Execution ─────────────────────────────────────

  private async executeNode(
    nodeId: string,
    flow: Flow,
    run: FlowRun,
    nodeStates: Map<string, NodeRunState>,
    triggerOutputs: Record<string, unknown> | undefined,
    abortController: AbortController,
  ): Promise<void> {
    const node = flow.nodes.find((n) => n.id === nodeId);
    if (!node) return;

    const state = nodeStates.get(nodeId);
    if (!state || state.status !== 'pending') return;

    const definition = this.registry.get(node.type);
    if (!definition) {
      state.status = 'failed';
      state.error = `Unknown node type: ${node.type}`;
      state.startedAt = Date.now();
      state.completedAt = Date.now();
      this.notifySubscribers(run);
      return;
    }

    // Mark as running
    state.status = 'running';
    state.startedAt = Date.now();
    this.notifySubscribers(run);

    // Resolve inputs from upstream nodes
    let inputs: Record<string, unknown>;
    if (definition.category === 'trigger' && triggerOutputs) {
      // Trigger nodes receive the trigger context as their inputs
      inputs = triggerOutputs;
    } else {
      inputs = this.resolveNodeInputs(nodeId, flow, nodeStates);
    }

    // Build execution context
    const context: NodeExecutionContext = {
      flowId: flow.id,
      runId: run.id,
      variables: new Map(flow.variables.map((v) => [v.name, v.defaultValue])),
      abortSignal: abortController.signal,
    };

    try {
      const outputs = await definition.execute(inputs, node.config, context);
      state.status = 'completed';
      state.outputs = outputs;
      state.completedAt = Date.now();
    } catch (err) {
      state.status = 'failed';
      state.error = err instanceof Error ? err.message : String(err);
      state.completedAt = Date.now();
    }

    this.notifySubscribers(run);
  }

  /**
   * Resolve inputs for a node by walking edges backwards to find source
   * node outputs.
   *
   * For each incoming edge, the source node's output port value is mapped
   * to the target node's input port name.
   */
  private resolveNodeInputs(
    nodeId: string,
    flow: Flow,
    nodeStates: Map<string, NodeRunState>,
  ): Record<string, unknown> {
    const inputs: Record<string, unknown> = {};

    for (const edge of flow.edges) {
      if (edge.targetNodeId !== nodeId) continue;

      const sourceState = nodeStates.get(edge.sourceNodeId);
      if (sourceState && sourceState.status === 'completed') {
        const outputValue = sourceState.outputs[edge.sourcePort];
        inputs[edge.targetPort] = outputValue;
      }
    }

    return inputs;
  }

  // ── Internal: Run State Management ───────────────────────────────

  /**
   * Mark all remaining pending nodes as skipped (used when a node
   * fails or the run is cancelled).
   */
  private markRemainingSkipped(
    run: FlowRun,
    nodeStates: Map<string, NodeRunState>,
  ): void {
    for (const state of nodeStates.values()) {
      if (state.status === 'pending') {
        state.status = 'skipped';
      }
    }
    this.notifySubscribers(run);
  }

  /** Finalize a run with a terminal status. */
  private finalizeRun(
    run: FlowRun,
    status: FlowRunStatus,
    error?: string,
  ): void {
    run.status = status;
    run.completedAt = Date.now();
    if (error) {
      run.error = error;
    }

    // Clean up abort controller
    this.abortControllers.delete(run.id);

    this.notifySubscribers(run);
  }

  private notifySubscribers(run: FlowRun): void {
    for (const fn of this.subscribers) fn(run);
  }
}
