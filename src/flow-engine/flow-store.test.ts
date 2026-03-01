import { describe, it, expect, beforeEach, vi } from 'vitest';
import { FlowStore } from './flow-store';
import type { Flow } from './types';

// Mock localStorage
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
  getItem: (key: string) => storage.get(key) ?? null,
  setItem: (key: string, value: string) => storage.set(key, value),
  removeItem: (key: string) => storage.delete(key),
});

describe('FlowStore', () => {
  let store: FlowStore;

  beforeEach(() => {
    storage.clear();
    store = new FlowStore();
  });

  describe('create', () => {
    it('creates a flow with auto-generated fields', () => {
      const flow = store.create({
        name: 'Test Flow',
        description: 'A test',
        tags: ['test'],
        enabled: true,
        nodes: [],
        edges: [],
        variables: [],
      });

      expect(flow.id).toBeTruthy();
      expect(flow.name).toBe('Test Flow');
      expect(flow.version).toBe(1);
      expect(flow.schemaVersion).toBe(1);
      expect(flow.createdAt).toBeGreaterThan(0);
      expect(flow.updatedAt).toBe(flow.createdAt);
    });

    it('persists to localStorage', () => {
      store.create({
        name: 'Persist',
        description: '',
        tags: [],
        enabled: true,
        nodes: [],
        edges: [],
        variables: [],
      });

      const raw = storage.get('godly-flows');
      expect(raw).toBeTruthy();
      const parsed = JSON.parse(raw!);
      expect(parsed).toHaveLength(1);
      expect(parsed[0].name).toBe('Persist');
    });
  });

  describe('getAll / getById', () => {
    it('returns all flows sorted by updatedAt desc', () => {
      const f1 = store.create({ name: 'A', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      // Update f1 so its updatedAt is later
      store.update(f1.id, { description: 'updated' });
      store.create({ name: 'B', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });

      const all = store.getAll();
      expect(all).toHaveLength(2);
      // f1 was updated after f2 was created, so f1 is first
      expect(all[0].id).toBe(f1.id);
    });

    it('retrieves flow by ID', () => {
      const flow = store.create({ name: 'Find Me', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      expect(store.getById(flow.id)?.name).toBe('Find Me');
    });

    it('returns undefined for unknown ID', () => {
      expect(store.getById('nonexistent')).toBeUndefined();
    });
  });

  describe('update', () => {
    it('updates name and bumps version', () => {
      const flow = store.create({ name: 'Original', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      const updated = store.update(flow.id, { name: 'Updated' });

      expect(updated?.name).toBe('Updated');
      expect(updated?.version).toBe(2);
      expect(updated?.updatedAt).toBeGreaterThanOrEqual(flow.updatedAt);
    });

    it('preserves identity fields', () => {
      const flow = store.create({ name: 'Test', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      const updated = store.update(flow.id, {
        id: 'hacked',
        schemaVersion: 99 as any,
        createdAt: 0,
      } as Partial<Flow>);

      expect(updated?.id).toBe(flow.id);
      expect(updated?.schemaVersion).toBe(1);
      expect(updated?.createdAt).toBe(flow.createdAt);
    });

    it('returns undefined for unknown ID', () => {
      expect(store.update('nonexistent', { name: 'Nope' })).toBeUndefined();
    });
  });

  describe('delete', () => {
    it('removes flow and returns true', () => {
      const flow = store.create({ name: 'Delete Me', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      expect(store.delete(flow.id)).toBe(true);
      expect(store.getById(flow.id)).toBeUndefined();
      expect(store.getAll()).toHaveLength(0);
    });

    it('returns false for unknown ID', () => {
      expect(store.delete('nonexistent')).toBe(false);
    });
  });

  describe('duplicate', () => {
    it('creates a copy with new ID and "(copy)" suffix', () => {
      const original = store.create({ name: 'Original', description: 'desc', tags: ['a'], enabled: true, nodes: [], edges: [], variables: [] });
      const copy = store.duplicate(original.id);

      expect(copy).toBeTruthy();
      expect(copy!.id).not.toBe(original.id);
      expect(copy!.name).toBe('Original (copy)');
      expect(copy!.version).toBe(1);
      expect(store.getAll()).toHaveLength(2);
    });

    it('returns undefined for unknown ID', () => {
      expect(store.duplicate('nonexistent')).toBeUndefined();
    });
  });

  describe('setEnabled', () => {
    it('toggles enabled state', () => {
      const flow = store.create({ name: 'Toggle', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      store.setEnabled(flow.id, false);
      expect(store.getById(flow.id)?.enabled).toBe(false);
    });
  });

  describe('import / export', () => {
    it('exports as pretty-printed JSON', () => {
      const flow = store.create({ name: 'Export', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      const json = store.exportFlow(flow.id);
      expect(json).toBeTruthy();
      const parsed = JSON.parse(json!);
      expect(parsed.name).toBe('Export');
    });

    it('imports valid JSON with new ID', () => {
      const original = store.create({ name: 'Original', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      const json = store.exportFlow(original.id)!;

      const imported = store.importFlow(json);
      expect(imported.id).not.toBe(original.id);
      expect(imported.name).toBe('Original');
      expect(imported.version).toBe(1);
      expect(store.getAll()).toHaveLength(2);
    });

    it('rejects invalid JSON', () => {
      expect(() => store.importFlow('not json')).toThrow(/Invalid JSON/);
    });

    it('rejects wrong schema version', () => {
      const json = JSON.stringify({ schemaVersion: 99, name: 'Bad', nodes: [], edges: [] });
      expect(() => store.importFlow(json)).toThrow(/Unsupported schema version/);
    });

    it('rejects missing required fields', () => {
      const json = JSON.stringify({ schemaVersion: 1 });
      expect(() => store.importFlow(json)).toThrow(/missing required fields/);
    });
  });

  describe('subscribe', () => {
    it('notifies on create', () => {
      const fn = vi.fn();
      store.subscribe(fn);
      store.create({ name: 'Sub', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      expect(fn).toHaveBeenCalledTimes(1);
    });

    it('unsubscribe stops notifications', () => {
      const fn = vi.fn();
      const unsub = store.subscribe(fn);
      unsub();
      store.create({ name: 'Sub', description: '', tags: [], enabled: true, nodes: [], edges: [], variables: [] });
      expect(fn).not.toHaveBeenCalled();
    });
  });

  describe('persistence', () => {
    it('loads flows from localStorage on construction', () => {
      // Pre-seed localStorage
      const flowData = [{
        id: 'pre-existing',
        name: 'Loaded',
        description: '',
        version: 1,
        schemaVersion: 1,
        tags: [],
        enabled: true,
        nodes: [],
        edges: [],
        variables: [],
        createdAt: 1000,
        updatedAt: 1000,
      }];
      storage.set('godly-flows', JSON.stringify(flowData));

      const freshStore = new FlowStore();
      expect(freshStore.getById('pre-existing')?.name).toBe('Loaded');
    });
  });
});
