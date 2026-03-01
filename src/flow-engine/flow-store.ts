import type { Flow } from './types';

// ── Persistence key ──────────────────────────────────────────────────

const STORAGE_KEY = 'godly-flows';
const CURRENT_SCHEMA_VERSION = 1;

// ── Serialization helpers ────────────────────────────────────────────

/** Shape of a Flow as stored in JSON (identical to Flow but typed loosely for parsing). */
interface StoredFlow {
  id: string;
  name: string;
  description: string;
  version: number;
  schemaVersion: number;
  tags: string[];
  enabled: boolean;
  nodes: Flow['nodes'];
  edges: Flow['edges'];
  variables: Flow['variables'];
  createdAt: number;
  updatedAt: number;
  author?: string;
}

// ── Store ────────────────────────────────────────────────────────────

type Subscriber = () => void;

/**
 * CRUD store for Godly Flows with localStorage persistence.
 *
 * Follows the same observable/subscribe pattern used by KeybindingStore
 * and other stores in the project.
 */
export class FlowStore {
  private flows: Map<string, Flow> = new Map();
  private subscribers: Set<Subscriber> = new Set();

  constructor() {
    this.loadFromStorage();
  }

  // ── Queries ──────────────────────────────────────────────────────

  /** Return all flows, ordered by updatedAt descending. */
  getAll(): Flow[] {
    return Array.from(this.flows.values()).sort(
      (a, b) => b.updatedAt - a.updatedAt,
    );
  }

  /** Look up a flow by ID. */
  getById(id: string): Flow | undefined {
    return this.flows.get(id);
  }

  // ── Mutations ────────────────────────────────────────────────────

  /**
   * Create a new flow. Caller provides everything except auto-generated
   * fields (id, version, schemaVersion, timestamps).
   */
  create(
    partial: Omit<Flow, 'id' | 'version' | 'schemaVersion' | 'createdAt' | 'updatedAt'>,
  ): Flow {
    const now = Date.now();
    const flow: Flow = {
      ...partial,
      id: crypto.randomUUID(),
      version: 1,
      schemaVersion: CURRENT_SCHEMA_VERSION as 1,
      createdAt: now,
      updatedAt: now,
    };
    this.flows.set(flow.id, flow);
    this.saveToStorage();
    this.notify();
    return flow;
  }

  /**
   * Partially update an existing flow. Bumps `version` and `updatedAt`
   * automatically. Returns the updated flow, or undefined if not found.
   */
  update(id: string, partial: Partial<Flow>): Flow | undefined {
    const existing = this.flows.get(id);
    if (!existing) return undefined;

    const updated: Flow = {
      ...existing,
      ...partial,
      // Preserve identity fields — callers cannot change these
      id: existing.id,
      schemaVersion: existing.schemaVersion,
      createdAt: existing.createdAt,
      // Bump metadata
      version: existing.version + 1,
      updatedAt: Date.now(),
    };

    this.flows.set(id, updated);
    this.saveToStorage();
    this.notify();
    return updated;
  }

  /** Delete a flow by ID. Returns true if it existed. */
  delete(id: string): boolean {
    const existed = this.flows.delete(id);
    if (existed) {
      this.saveToStorage();
      this.notify();
    }
    return existed;
  }

  /**
   * Duplicate an existing flow with a new ID, reset version, and
   * " (copy)" appended to the name. Returns the new flow, or undefined
   * if the source flow was not found.
   */
  duplicate(id: string): Flow | undefined {
    const source = this.flows.get(id);
    if (!source) return undefined;

    const now = Date.now();
    const copy: Flow = {
      ...structuredClone(source),
      id: crypto.randomUUID(),
      name: `${source.name} (copy)`,
      version: 1,
      createdAt: now,
      updatedAt: now,
    };

    this.flows.set(copy.id, copy);
    this.saveToStorage();
    this.notify();
    return copy;
  }

  // ── Enable / Disable ─────────────────────────────────────────────

  /** Toggle a flow's enabled state. */
  setEnabled(id: string, enabled: boolean): void {
    const flow = this.flows.get(id);
    if (!flow) return;
    this.update(id, { enabled });
  }

  // ── Import / Export ──────────────────────────────────────────────

  /** Serialize a flow to a JSON string for sharing/backup. */
  exportFlow(id: string): string | undefined {
    const flow = this.flows.get(id);
    if (!flow) return undefined;
    return JSON.stringify(flow, null, 2);
  }

  /**
   * Import a flow from a JSON string.
   *
   * Validates that `schemaVersion` matches the current version, assigns
   * a new UUID (to avoid collisions), and resets timestamps.
   *
   * Throws if the JSON is malformed or the schema version is unsupported.
   */
  importFlow(json: string): Flow {
    let parsed: StoredFlow;
    try {
      parsed = JSON.parse(json) as StoredFlow;
    } catch {
      throw new Error('Invalid JSON: could not parse flow data');
    }

    if (!parsed || typeof parsed !== 'object') {
      throw new Error('Invalid flow data: expected an object');
    }

    if (parsed.schemaVersion !== CURRENT_SCHEMA_VERSION) {
      throw new Error(
        `Unsupported schema version: expected ${CURRENT_SCHEMA_VERSION}, got ${parsed.schemaVersion}`,
      );
    }

    if (!parsed.name || !Array.isArray(parsed.nodes) || !Array.isArray(parsed.edges)) {
      throw new Error('Invalid flow data: missing required fields (name, nodes, edges)');
    }

    const now = Date.now();
    const flow: Flow = {
      name: parsed.name,
      description: parsed.description ?? '',
      tags: Array.isArray(parsed.tags) ? parsed.tags : [],
      enabled: parsed.enabled ?? true,
      nodes: parsed.nodes,
      edges: parsed.edges,
      variables: Array.isArray(parsed.variables) ? parsed.variables : [],
      author: parsed.author,
      // Generated fields
      id: crypto.randomUUID(),
      version: 1,
      schemaVersion: CURRENT_SCHEMA_VERSION as 1,
      createdAt: now,
      updatedAt: now,
    };

    this.flows.set(flow.id, flow);
    this.saveToStorage();
    this.notify();
    return flow;
  }

  // ── Observable ───────────────────────────────────────────────────

  /** Subscribe to store changes. Returns an unsubscribe function. */
  subscribe(fn: Subscriber): () => void {
    this.subscribers.add(fn);
    return () => {
      this.subscribers.delete(fn);
    };
  }

  private notify(): void {
    for (const fn of this.subscribers) fn();
  }

  // ── Persistence ──────────────────────────────────────────────────

  private saveToStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const data = Array.from(this.flows.values());
      localStorage.setItem(STORAGE_KEY, JSON.stringify(data));
    } catch {
      // No localStorage available — silently skip
    }
  }

  private loadFromStorage(): void {
    try {
      if (typeof localStorage === 'undefined') return;
      const raw = localStorage.getItem(STORAGE_KEY);
      if (!raw) return;
      const data: StoredFlow[] = JSON.parse(raw);
      if (!Array.isArray(data)) return;

      for (const stored of data) {
        if (
          stored &&
          typeof stored === 'object' &&
          typeof stored.id === 'string' &&
          stored.schemaVersion === CURRENT_SCHEMA_VERSION
        ) {
          this.flows.set(stored.id, stored as Flow);
        }
      }
    } catch {
      // Corrupt data — start fresh
      console.warn('[FlowStore] Failed to load flows from localStorage, starting fresh');
    }
  }
}

// ── Singleton ────────────────────────────────────────────────────────

export const flowStore = new FlowStore();
