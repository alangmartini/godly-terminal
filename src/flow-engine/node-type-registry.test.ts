import { describe, it, expect, beforeEach } from 'vitest';
import { NodeTypeRegistry } from './node-type-registry';
import type { NodeTypeDefinition } from './types';

function makeDef(type: string, category: NodeTypeDefinition['category'] = 'terminal'): NodeTypeDefinition {
  return {
    type,
    category,
    label: type,
    description: '',
    icon: '',
    ports: [],
    configSchema: [],
    execute: async () => ({}),
  };
}

describe('NodeTypeRegistry', () => {
  let registry: NodeTypeRegistry;

  beforeEach(() => {
    registry = new NodeTypeRegistry();
  });

  it('registers and retrieves a definition', () => {
    const def = makeDef('terminal.create');
    registry.register(def);
    expect(registry.get('terminal.create')).toBe(def);
  });

  it('returns undefined for unregistered type', () => {
    expect(registry.get('nonexistent')).toBeUndefined();
  });

  it('has() checks existence', () => {
    registry.register(makeDef('test.node'));
    expect(registry.has('test.node')).toBe(true);
    expect(registry.has('nope')).toBe(false);
  });

  it('getAll() returns all definitions', () => {
    registry.register(makeDef('a'));
    registry.register(makeDef('b'));
    expect(registry.getAll()).toHaveLength(2);
  });

  it('getByCategory() filters by category', () => {
    registry.register(makeDef('t.hotkey', 'trigger'));
    registry.register(makeDef('term.create', 'terminal'));
    registry.register(makeDef('term.close', 'terminal'));

    expect(registry.getByCategory('trigger')).toHaveLength(1);
    expect(registry.getByCategory('terminal')).toHaveLength(2);
    expect(registry.getByCategory('data')).toHaveLength(0);
  });

  it('overwrites existing definition with same type', () => {
    const def1 = makeDef('test.node');
    const def2 = makeDef('test.node');
    def2.label = 'Updated';

    registry.register(def1);
    registry.register(def2);

    expect(registry.getAll()).toHaveLength(1);
    expect(registry.get('test.node')?.label).toBe('Updated');
  });
});
