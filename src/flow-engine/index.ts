import { flowStore } from './flow-store';
import { FlowExecutor } from './flow-executor';
import { FlowTriggerManager } from './flow-trigger-manager';
import { nodeTypeRegistry } from './node-type-registry';
import { registerAllNodes } from './nodes/index';

// ── Flow Engine Initialization ──────────────────────────────────────

let executor: FlowExecutor | null = null;
let triggerManager: FlowTriggerManager | null = null;

/**
 * Initialize the flow engine. Must be called once during app startup,
 * after plugins are loaded.
 *
 * 1. Registers all built-in node types
 * 2. Creates the executor
 * 3. Sets up the trigger manager with hotkey listeners
 * 4. Subscribes to store changes for trigger refresh
 * 5. Exposes the engine on `window.__FLOW_ENGINE__` for MCP access
 */
export function initFlowEngine(): void {
  // 1. Register all node types
  registerAllNodes();

  // 2. Create executor
  executor = new FlowExecutor(nodeTypeRegistry);

  // 3. Create trigger manager and start listening
  triggerManager = new FlowTriggerManager();
  triggerManager.start((flowId) => {
    const flow = flowStore.getById(flowId);
    if (flow && flow.enabled) {
      executor!.startRun(flow).catch((err) =>
        console.warn('[Flows] Hotkey-triggered run failed:', err),
      );
    }
  });

  // 4. Register all current flows for hotkey triggers
  triggerManager.refreshAll(flowStore.getAll());

  // 5. Subscribe to store changes to keep triggers in sync
  flowStore.subscribe(() => {
    triggerManager!.refreshAll(flowStore.getAll());
  });

  // 6. Expose for MCP execute_js access
  (window as any).__FLOW_ENGINE__ = {
    flowStore,
    executor,
    triggerManager,
    nodeTypeRegistry,
    triggerFlow: async (flowId: string, params?: Record<string, unknown>) => {
      const flow = flowStore.getById(flowId);
      if (!flow) throw new Error(`Flow not found: ${flowId}`);
      return executor!.startRun(flow, params);
    },
  };

  console.info('[Flows] Flow engine initialized');
}

/** Get the flow executor instance (null before init). */
export function getFlowExecutor(): FlowExecutor | null {
  return executor;
}

/** Get the flow trigger manager instance (null before init). */
export function getFlowTriggerManager(): FlowTriggerManager | null {
  return triggerManager;
}
