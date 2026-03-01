import { nodeTypeRegistry } from '../node-type-registry';
import { triggerNodes } from './trigger-nodes';
import { terminalNodes } from './terminal-nodes';
import { dataNodes } from './data-nodes';

/**
 * Register all built-in node type definitions with the global registry.
 * Call this once at application startup before any flow execution.
 */
export function registerAllNodes(): void {
  for (const nodes of [triggerNodes, terminalNodes, dataNodes]) {
    for (const node of nodes) {
      nodeTypeRegistry.register(node);
    }
  }
}
