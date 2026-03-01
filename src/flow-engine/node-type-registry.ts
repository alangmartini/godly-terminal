import type { NodeCategory, NodeTypeDefinition } from './types';

// ── Node Type Registry ───────────────────────────────────────────────

/**
 * Singleton registry that maps node type strings (e.g. "terminal.create")
 * to their full NodeTypeDefinition, including ports, config schema, and
 * execute function.
 *
 * Node implementations call `nodeTypeRegistry.register(definition)` at
 * module load time. The flow executor and editor UI read definitions
 * from the registry at runtime.
 */
export class NodeTypeRegistry {
  private definitions: Map<string, NodeTypeDefinition> = new Map();

  /**
   * Register a node type definition. Overwrites any existing definition
   * with the same `type` string (allows hot-reload during development).
   */
  register(definition: NodeTypeDefinition): void {
    if (!definition.type) {
      console.warn('[NodeTypeRegistry] Skipping registration: definition has no type');
      return;
    }
    this.definitions.set(definition.type, definition);
  }

  /** Look up a definition by its type string. */
  get(type: string): NodeTypeDefinition | undefined {
    return this.definitions.get(type);
  }

  /** Return all registered definitions. */
  getAll(): NodeTypeDefinition[] {
    return Array.from(this.definitions.values());
  }

  /** Return all definitions belonging to a specific category. */
  getByCategory(category: NodeCategory): NodeTypeDefinition[] {
    return this.getAll().filter((def) => def.category === category);
  }

  /** Check whether a type string has a registered definition. */
  has(type: string): boolean {
    return this.definitions.has(type);
  }
}

// ── Singleton ────────────────────────────────────────────────────────

export const nodeTypeRegistry = new NodeTypeRegistry();
