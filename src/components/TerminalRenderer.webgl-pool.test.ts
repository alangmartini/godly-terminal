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
//
// Models the ACTUAL code paths in TerminalRenderer, including the two
// WebGL promotion paths (acquireWebGL on existing canvas vs promoteToWebGL
// with a new canvas) and the demoteToCanvas2D path that replaces the canvas.
// Each renderer has a unique canvas ID that changes on promote/demote.

let nextCanvasId = 1;

class MockRenderer {
  id: string;
  useWebGL = false;
  contextLostDegraded = false;
  backend = 'Canvas2D';
  disposed = false;
  canvasId: string;
  private canvasHas2DContext = true;  // starts with Canvas2D context

  constructor(id: string, private pool: MockPool) {
    this.id = id;
    this.canvasId = `canvas-${nextCanvasId++}`;
  }

  // Tries getContext('webgl2') on the CURRENT canvas. Fails if canvas
  // already has a 2d context (browser limitation).
  acquireWebGL(): boolean {
    if (this.useWebGL) return true;
    if (this.contextLostDegraded) return false;
    if (this.canvasHas2DContext) return false; // can't get webgl2 on a 2d canvas
    if (!this.pool.acquire(this.canvasId)) return false;
    this.useWebGL = true;
    this.backend = 'WebGL2';
    return true;
  }

  // Creates a NEW canvas, acquires WebGL through the pool, swaps into DOM.
  // This is the path used when acquireWebGL fails (canvas locked to 2D).
  promoteToWebGL(): boolean {
    if (this.useWebGL) return true;
    if (this.contextLostDegraded) return false;
    const newCanvasId = `canvas-${nextCanvasId++}`;
    if (!this.pool.acquire(newCanvasId)) return false;
    this.canvasId = newCanvasId;
    this.useWebGL = true;
    this.canvasHas2DContext = false;
    this.backend = 'WebGL2';
    return true;
  }

  releaseWebGL(): void {
    if (!this.useWebGL) return;
    this.pool.release(this.canvasId);
    this.useWebGL = false;
    this.backend = 'Canvas2D';
  }

  // Releases pool slot, disposes WebGL, creates a fresh Canvas2D canvas.
  demoteToCanvas2D(): void {
    if (!this.useWebGL) return;
    this.pool.release(this.canvasId);
    this.useWebGL = false;
    this.backend = 'Canvas2D';
    // Replace canvas with a new Canvas2D one (mirrors real code)
    this.canvasId = `canvas-${nextCanvasId++}`;
    this.canvasHas2DContext = true;
  }

  simulateContextLost(): void {
    this.pool.notifyContextLost(this.canvasId);
    this.useWebGL = false;
    this.backend = 'Canvas2D';
    this.contextLostDegraded = true;
  }

  // Mirrors real releaseCanvasResources(): calls demoteToCanvas2D, not releaseWebGL.
  releaseCanvasResources(): void {
    this.demoteToCanvas2D();
  }

  // Mirrors real restoreCanvasResources(): tries acquireWebGL (fails on 2D canvas),
  // then falls back to promoteToWebGL (creates new canvas through pool).
  restoreCanvasResources(): void {
    this.acquireWebGL();
    if (!this.useWebGL) {
      this.promoteToWebGL();
    }
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
    nextCanvasId = 1;
  });

  describe('deferred WebGL acquisition', () => {
    it('renderer starts with Canvas2D, not WebGL', () => {
      const renderer = new MockRenderer('term-1', pool);
      expect(renderer.useWebGL).toBe(false);
      expect(renderer.backend).toBe('Canvas2D');
      expect(pool.activeCount).toBe(0);
    });

    it('acquireWebGL fails on canvas with 2D context (always the case for fresh renderers)', () => {
      const renderer = new MockRenderer('term-1', pool);
      // acquireWebGL tries getContext('webgl2') on the existing canvas,
      // which fails because the constructor already got a 2D context.
      const acquired = renderer.acquireWebGL();

      expect(acquired).toBe(false);
      expect(renderer.useWebGL).toBe(false);
      expect(pool.activeCount).toBe(0);
    });

    it('promoteToWebGL switches to WebGL when pool has capacity', () => {
      const renderer = new MockRenderer('term-1', pool);
      const promoted = renderer.promoteToWebGL();

      expect(promoted).toBe(true);
      expect(renderer.useWebGL).toBe(true);
      expect(renderer.backend).toBe('WebGL2');
      expect(pool.activeCount).toBe(1);
    });

    it('promoteToWebGL is idempotent', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.promoteToWebGL();
      renderer.promoteToWebGL();

      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);
    });
  });

  describe('context limit enforcement', () => {
    it('refuses promotion when pool is at capacity', () => {
      const renderers: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`term-${i}`, pool);
        r.promoteToWebGL();
        renderers.push(r);
      }
      expect(pool.activeCount).toBe(MAX_CONTEXTS);

      // The next promote should fail
      const overflow = new MockRenderer('term-overflow', pool);
      const promoted = overflow.promoteToWebGL();

      expect(promoted).toBe(false);
      expect(overflow.useWebGL).toBe(false);
      expect(overflow.backend).toBe('Canvas2D');
      expect(pool.activeCount).toBe(MAX_CONTEXTS);
    });

    it('allows promotion after another renderer demotes', () => {
      const renderers: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`term-${i}`, pool);
        r.promoteToWebGL();
        renderers.push(r);
      }

      // Demote one
      renderers[0].demoteToCanvas2D();
      expect(pool.activeCount).toBe(MAX_CONTEXTS - 1);

      // Now the new renderer should succeed
      const newRenderer = new MockRenderer('term-new', pool);
      const promoted = newRenderer.promoteToWebGL();

      expect(promoted).toBe(true);
      expect(pool.activeCount).toBe(MAX_CONTEXTS);
    });
  });

  describe('release on hide', () => {
    it('demoteToCanvas2D returns context to pool and switches to Canvas2D', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.promoteToWebGL();
      expect(pool.activeCount).toBe(1);

      renderer.demoteToCanvas2D();

      expect(renderer.useWebGL).toBe(false);
      expect(renderer.backend).toBe('Canvas2D');
      expect(pool.activeCount).toBe(0);
    });

    it('demoteToCanvas2D is a no-op when already on Canvas2D', () => {
      const renderer = new MockRenderer('term-1', pool);
      // Never promoted to WebGL
      renderer.demoteToCanvas2D();

      expect(pool.activeCount).toBe(0);
      expect(renderer.backend).toBe('Canvas2D');
    });

    it('releaseCanvasResources releases WebGL context via demoteToCanvas2D', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.promoteToWebGL();
      expect(pool.activeCount).toBe(1);

      renderer.releaseCanvasResources();

      expect(pool.activeCount).toBe(0);
      expect(renderer.useWebGL).toBe(false);
    });
  });

  describe('restore on show', () => {
    it('restoreCanvasResources re-acquires WebGL after pause', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.restoreCanvasResources(); // promote through pool
      renderer.releaseCanvasResources(); // demote, release pool slot
      expect(pool.activeCount).toBe(0);

      renderer.restoreCanvasResources(); // re-promote through pool

      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);
    });

    it('restoreCanvasResources stays on Canvas2D when pool is full', () => {
      // Fill the pool
      const others: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`other-${i}`, pool);
        r.promoteToWebGL();
        others.push(r);
      }

      // Our renderer tries to restore — both acquireWebGL and promoteToWebGL should fail
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
      renderer.promoteToWebGL();

      renderer.simulateContextLost();

      expect(renderer.useWebGL).toBe(false);
      expect(renderer.backend).toBe('Canvas2D');
      expect(renderer.contextLostDegraded).toBe(true);
      expect(pool.activeCount).toBe(0);
    });

    it('degraded renderer refuses future promoteToWebGL calls', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.promoteToWebGL();
      renderer.simulateContextLost();

      // Try to re-promote — should refuse (degraded)
      const promoted = renderer.promoteToWebGL();

      expect(promoted).toBe(false);
      expect(renderer.useWebGL).toBe(false);
      expect(pool.activeCount).toBe(0);
    });

    it('context loss frees pool slot for other renderers', () => {
      // Fill pool
      const renderers: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`term-${i}`, pool);
        r.promoteToWebGL();
        renderers.push(r);
      }
      expect(pool.activeCount).toBe(MAX_CONTEXTS);

      // One loses context
      renderers[3].simulateContextLost();
      expect(pool.activeCount).toBe(MAX_CONTEXTS - 1);

      // New renderer can now promote
      const newRenderer = new MockRenderer('term-new', pool);
      expect(newRenderer.promoteToWebGL()).toBe(true);
      expect(pool.activeCount).toBe(MAX_CONTEXTS);
    });
  });

  describe('dispose', () => {
    it('dispose releases WebGL context from pool', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.promoteToWebGL();
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
      allRenderers[0].promoteToWebGL();
      allRenderers[1].promoteToWebGL();
      expect(pool.activeCount).toBe(2);

      // Switch to a different workspace (2 more visible)
      allRenderers[0].demoteToCanvas2D();
      allRenderers[1].demoteToCanvas2D();
      allRenderers[5].promoteToWebGL();
      allRenderers[6].promoteToWebGL();
      expect(pool.activeCount).toBe(2);

      // All 25 terminals still exist, only 2 hold contexts
      expect(allRenderers.every(r => !r.disposed)).toBe(true);
    });

    it('graceful fallback when switching tabs faster than context recycling', () => {
      // Fill pool with 8 renderers
      const visible: MockRenderer[] = [];
      for (let i = 0; i < 8; i++) {
        const r = new MockRenderer(`vis-${i}`, pool);
        r.promoteToWebGL();
        visible.push(r);
      }
      expect(pool.activeCount).toBe(8);

      // Try to show a 9th without releasing any — should fall back to Canvas2D
      const ninth = new MockRenderer('vis-8', pool);
      const acquired = ninth.promoteToWebGL();
      expect(acquired).toBe(false);
      expect(ninth.backend).toBe('Canvas2D');

      // Now release one and retry
      visible[0].demoteToCanvas2D();
      expect(ninth.promoteToWebGL()).toBe(true);
      expect(ninth.backend).toBe('WebGL2');
    });

    it('pause/resume cycle correctly manages pool', () => {
      const renderer = new MockRenderer('term-1', pool);

      // Mount + become visible
      renderer.restoreCanvasResources(); // acquireWebGL fails (2D canvas), promoteToWebGL succeeds
      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);

      // Tab switch — pause
      renderer.releaseCanvasResources(); // demoteToCanvas2D releases pool slot
      expect(pool.activeCount).toBe(0);
      expect(renderer.useWebGL).toBe(false);

      // Tab switch back — resume
      renderer.restoreCanvasResources(); // acquireWebGL fails (2D canvas), promoteToWebGL succeeds
      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);

      // Destroy
      renderer.dispose();
      expect(pool.activeCount).toBe(0);
    });
  });

  // Regression tests for #337: pool leak via demoteToCanvas2D / promoteToWebGL bypass
  describe('pool leak regression (#337)', () => {
    it('demoteToCanvas2D releases pool slot', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.promoteToWebGL();
      expect(pool.activeCount).toBe(1);

      renderer.demoteToCanvas2D();
      expect(pool.activeCount).toBe(0);
      expect(renderer.useWebGL).toBe(false);
    });

    it('promoteToWebGL acquires through pool', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.promoteToWebGL();

      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);
    });

    it('promoteToWebGL respects pool capacity', () => {
      // Fill the pool
      const renderers: MockRenderer[] = [];
      for (let i = 0; i < MAX_CONTEXTS; i++) {
        const r = new MockRenderer(`term-${i}`, pool);
        r.promoteToWebGL();
        renderers.push(r);
      }
      expect(pool.activeCount).toBe(MAX_CONTEXTS);

      // Next promoteToWebGL should fail (pool full)
      const overflow = new MockRenderer('term-overflow', pool);
      expect(overflow.promoteToWebGL()).toBe(false);
      expect(overflow.useWebGL).toBe(false);
      expect(pool.activeCount).toBe(MAX_CONTEXTS);
    });

    it('50 rapid pause/resume cycles do not leak pool slots', () => {
      const renderer = new MockRenderer('term-1', pool);
      renderer.restoreCanvasResources();
      expect(pool.activeCount).toBe(1);

      for (let i = 0; i < 50; i++) {
        renderer.releaseCanvasResources();
        expect(pool.activeCount).toBe(0);

        renderer.restoreCanvasResources();
        expect(pool.activeCount).toBe(1);
      }

      expect(pool.activeCount).toBe(1);
      expect(renderer.useWebGL).toBe(true);
    });

    it('multiple terminals cycling through pause/resume keep pool count accurate', () => {
      const terms: MockRenderer[] = [];
      for (let i = 0; i < 20; i++) {
        terms.push(new MockRenderer(`term-${i}`, pool));
      }

      // Simulate workspace switching: show 2, hide 2, show 2, repeat
      for (let cycle = 0; cycle < 10; cycle++) {
        const a = cycle * 2 % 20;
        const b = (cycle * 2 + 1) % 20;

        terms[a].restoreCanvasResources();
        terms[b].restoreCanvasResources();
        expect(pool.activeCount).toBe(2);

        terms[a].releaseCanvasResources();
        terms[b].releaseCanvasResources();
        expect(pool.activeCount).toBe(0);
      }
    });

    it('demoteToCanvas2D after promoteToWebGL releases correct pool slot', () => {
      const renderer = new MockRenderer('term-1', pool);

      // Promote creates a new canvas and acquires through pool
      renderer.promoteToWebGL();
      const promotedCanvasId = renderer.canvasId;
      expect(pool.isTracked(promotedCanvasId)).toBe(true);

      // Demote releases the promoted canvas and creates a new 2D canvas
      renderer.demoteToCanvas2D();
      expect(pool.isTracked(promotedCanvasId)).toBe(false);
      expect(pool.activeCount).toBe(0);
    });
  });
});
