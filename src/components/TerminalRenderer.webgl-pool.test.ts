import { describe, it, expect, beforeEach, vi } from 'vitest';

/**
 * Tests for WebGL context pooling in TerminalRenderer.
 *
 * Since jsdom does not provide real WebGL2 contexts, these tests verify the
 * pool logic and renderer state transitions using a simulator that mirrors
 * the actual acquire/release/context-loss code paths in TerminalRenderer.
 *
 * Bug: #312 — WebGL context exhaustion with 25+ terminals
 */

// ---- Pool simulator (mirrors WebGLContextPool logic) ----

const MAX_CONTEXTS = 8;

class MockPool {
  private tracked = new Set<string>();

  get activeCount() { return this.tracked.size; }
  get maxContexts() { return MAX_CONTEXTS; }

  canAcquire(): boolean { return this.tracked.size < MAX_CONTEXTS; }

  acquire(id: string): boolean {
    if (this.tracked.has(id)) return true;
    if (!this.canAcquire()) return false;
    this.tracked.add(id);
    return true;
  }

  release(id: string): void { this.tracked.delete(id); }

  notifyContextLost(id: string): void { this.tracked.delete(id); }

  isTracked(id: string): boolean { return this.tracked.has(id); }

  reset(): void { this.tracked.clear(); }
}

// ---- Renderer simulator (mirrors TerminalRenderer lifecycle) ----

class MockRenderer {
  id: string;
  useWebGL = false;
  contextLostDegraded = false;
  backend = 'Canvas2D';
  disposed = false;

  constructor(id: string, private pool: MockPool) {
    this.id = id;
    // Starts with Canvas2D (no WebGL in constructor)
  }

  acquireWebGL(): boolean {
    if (this.useWebGL) return true;
    if (this.contextLostDegraded) return false;
    if (!this.pool.acquire(this.id)) return false;
    this.useWebGL = true;
    this.backend = 'WebGL2';
    return true;
  }

  releaseWebGL(): void {
    if (!this.useWebGL) return;
    this.pool.release(this.id);
    this.useWebGL = false;
    this.backend = 'Canvas2D';
  }

  simulateContextLost(): void {
    this.pool.notifyContextLost(this.id);
    this.useWebGL = false;
    this.backend = 'Canvas2D';
    this.contextLostDegraded = true;
  }

  releaseCanvasResources(): void {
    if (this.useWebGL) {
      this.releaseWebGL();
    }
  }

  restoreCanvasResources(): void {
    this.acquireWebGL();
  }

  dispose(): void {
    if (this.useWebGL) {
      this.releaseWebGL();
    }
    this.disposed = true;
  }
}

// ---- Tests ----

describe('WebGL context pooling (TerminalRenderer integration)', () => {
  let pool: MockPool;

  beforeEach(() => {
    pool = new MockPool();
  });

  describe('deferred WebGL acquisition', () => {
    it('renderer starts with Canvas2D, not WebGL', () => {
      const renderer = new MockRenderer('term-1', pool);
      expect(renderer.useWebGL).toBe(false);
      expect(renderer.backend).toBe('Canvas2D');
      expect(pool.activeCount).toBe(0);
    });

    it('acquireWebGL switches to WebGL when pool has capacity', () => {
      const renderer = new MockRenderer('term-1', pool);
      const acquired = renderer.acquireWebGL();

      expect(acquired).toBe(true);
      expect(renderer.useWebGL).toBe(true);
      expect(renderer.backend).toBe('WebGL2');
      expect(pool.activeCount).toBe(1);
    });

    it('acquireWebGL is idempotent', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.acquireWebGL();
      renderer.acquireWebGL();

      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);
    });
  });

  describe('context limit enforcement', () => {
    it('refuses acquisition when pool is at capacity', () => {
      const renderers: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`term-${i}`, pool);
        r.acquireWebGL();
        renderers.push(r);
      }
      expect(pool.activeCount).toBe(MAX_CONTEXTS);

      // The next acquire should fail
      const overflow = new MockRenderer('term-overflow', pool);
      const acquired = overflow.acquireWebGL();

      expect(acquired).toBe(false);
      expect(overflow.useWebGL).toBe(false);
      expect(overflow.backend).toBe('Canvas2D');
      expect(pool.activeCount).toBe(MAX_CONTEXTS);
    });

    it('allows acquisition after another renderer releases', () => {
      const renderers: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`term-${i}`, pool);
        r.acquireWebGL();
        renderers.push(r);
      }

      // Release one
      renderers[0].releaseWebGL();
      expect(pool.activeCount).toBe(MAX_CONTEXTS - 1);

      // Now the new renderer should succeed
      const newRenderer = new MockRenderer('term-new', pool);
      const acquired = newRenderer.acquireWebGL();

      expect(acquired).toBe(true);
      expect(pool.activeCount).toBe(MAX_CONTEXTS);
    });
  });

  describe('release on hide', () => {
    it('releaseWebGL returns context to pool and switches to Canvas2D', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.acquireWebGL();
      expect(pool.activeCount).toBe(1);

      renderer.releaseWebGL();

      expect(renderer.useWebGL).toBe(false);
      expect(renderer.backend).toBe('Canvas2D');
      expect(pool.activeCount).toBe(0);
    });

    it('releaseWebGL is a no-op when already on Canvas2D', () => {
      const renderer = new MockRenderer('term-1', pool);
      // Never acquired WebGL
      renderer.releaseWebGL();

      expect(pool.activeCount).toBe(0);
      expect(renderer.backend).toBe('Canvas2D');
    });

    it('releaseCanvasResources releases WebGL context', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.acquireWebGL();
      expect(pool.activeCount).toBe(1);

      renderer.releaseCanvasResources();

      expect(pool.activeCount).toBe(0);
      expect(renderer.useWebGL).toBe(false);
    });
  });

  describe('restore on show', () => {
    it('restoreCanvasResources re-acquires WebGL', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.acquireWebGL();
      renderer.releaseCanvasResources();
      expect(pool.activeCount).toBe(0);

      renderer.restoreCanvasResources();

      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);
    });

    it('restoreCanvasResources stays on Canvas2D when pool is full', () => {
      // Fill the pool
      const others: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`other-${i}`, pool);
        r.acquireWebGL();
        others.push(r);
      }

      // Our renderer was never acquired, now tries to restore
      const renderer = new MockRenderer('term-victim', pool);
      renderer.restoreCanvasResources();

      expect(renderer.useWebGL).toBe(false);
      expect(renderer.backend).toBe('Canvas2D');
      expect(pool.activeCount).toBe(MAX_CONTEXTS);
    });
  });

  describe('context loss handling', () => {
    it('context loss switches to Canvas2D and marks degraded', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.acquireWebGL();

      renderer.simulateContextLost();

      expect(renderer.useWebGL).toBe(false);
      expect(renderer.backend).toBe('Canvas2D');
      expect(renderer.contextLostDegraded).toBe(true);
      expect(pool.activeCount).toBe(0);
    });

    it('degraded renderer refuses future acquireWebGL calls', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.acquireWebGL();
      renderer.simulateContextLost();

      // Try to re-acquire
      const acquired = renderer.acquireWebGL();

      expect(acquired).toBe(false);
      expect(renderer.useWebGL).toBe(false);
      expect(pool.activeCount).toBe(0);
    });

    it('context loss frees pool slot for other renderers', () => {
      // Fill pool
      const renderers: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`term-${i}`, pool);
        r.acquireWebGL();
        renderers.push(r);
      }
      expect(pool.activeCount).toBe(MAX_CONTEXTS);

      // One loses context
      renderers[3].simulateContextLost();
      expect(pool.activeCount).toBe(MAX_CONTEXTS - 1);

      // New renderer can now acquire
      const newRenderer = new MockRenderer('term-new', pool);
      expect(newRenderer.acquireWebGL()).toBe(true);
      expect(pool.activeCount).toBe(MAX_CONTEXTS);
    });
  });

  describe('dispose', () => {
    it('dispose releases WebGL context from pool', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.acquireWebGL();
      expect(pool.activeCount).toBe(1);

      renderer.dispose();

      expect(pool.activeCount).toBe(0);
      expect(renderer.disposed).toBe(true);
    });

    it('dispose is safe when already on Canvas2D', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.dispose();

      expect(pool.activeCount).toBe(0);
      expect(renderer.disposed).toBe(true);
    });
  });

  describe('realistic multi-terminal workflow', () => {
    it('supports 25+ terminals with only visible ones holding WebGL', () => {
      const allRenderers: MockRenderer[] = [];
      for (let i = 0; i < 25; i++) {
        allRenderers.push(new MockRenderer(`term-${i}`, pool));
      }

      // Initially, all start on Canvas2D
      expect(pool.activeCount).toBe(0);
      for (const r of allRenderers) {
        expect(r.useWebGL).toBe(false);
      }

      // Make first 2 visible (active tab + split)
      allRenderers[0].acquireWebGL();
      allRenderers[1].acquireWebGL();
      expect(pool.activeCount).toBe(2);

      // Switch to a different workspace (2 more visible)
      allRenderers[0].releaseWebGL();
      allRenderers[1].releaseWebGL();
      allRenderers[5].acquireWebGL();
      allRenderers[6].acquireWebGL();
      expect(pool.activeCount).toBe(2);

      // All 25 terminals still exist, only 2 hold contexts
      expect(allRenderers.every(r => !r.disposed)).toBe(true);
    });

    it('graceful fallback when switching tabs faster than context recycling', () => {
      // Fill pool with 8 renderers
      const visible: MockRenderer[] = [];
      for (let i = 0; i < 8; i++) {
        const r = new MockRenderer(`vis-${i}`, pool);
        r.acquireWebGL();
        visible.push(r);
      }
      expect(pool.activeCount).toBe(8);

      // Try to show a 9th without releasing any — should fall back to Canvas2D
      const ninth = new MockRenderer('vis-8', pool);
      const acquired = ninth.acquireWebGL();
      expect(acquired).toBe(false);
      expect(ninth.backend).toBe('Canvas2D');

      // Now release one and retry
      visible[0].releaseWebGL();
      expect(ninth.acquireWebGL()).toBe(true);
      expect(ninth.backend).toBe('WebGL2');
    });

    it('pause/resume cycle correctly manages pool', () => {
      const renderer = new MockRenderer('term-1', pool);

      // Mount + become visible
      renderer.restoreCanvasResources(); // acquires WebGL
      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);

      // Tab switch — pause
      renderer.releaseCanvasResources();
      expect(pool.activeCount).toBe(0);
      expect(renderer.useWebGL).toBe(false);

      // Tab switch back — resume
      renderer.restoreCanvasResources();
      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);

      // Destroy
      renderer.dispose();
      expect(pool.activeCount).toBe(0);
    });
  });
});
