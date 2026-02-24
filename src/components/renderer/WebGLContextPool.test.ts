import { describe, it, expect, beforeEach } from 'vitest';
import { webGLContextPool } from './WebGLContextPool';

/**
 * Tests for WebGLContextPool.
 *
 * Since we run in a node environment (no real DOM/WebGL), we test the pool's
 * tracking logic using mock canvas objects. The pool uses a Set<HTMLCanvasElement>
 * internally, so any object with identity equality works for tracking tests.
 *
 * Actual WebGL context creation returns null in this environment, so we focus
 * on the pool management logic (limit enforcement, tracking, release).
 */

// Create a mock "canvas" that satisfies the type signature.
// getContext('webgl2') returns null (same as a real browser that refuses).
function mockCanvas(id?: string): HTMLCanvasElement {
  return {
    _id: id,
    getContext: () => null,
  } as unknown as HTMLCanvasElement;
}

// Create a mock canvas whose getContext returns a fake WebGL2 context.
function mockCanvasWithGL(id?: string): { canvas: HTMLCanvasElement; gl: object } {
  const gl = { _id: id };
  const canvas = {
    _id: id,
    getContext: () => gl,
  } as unknown as HTMLCanvasElement;
  return { canvas, gl };
}

describe('WebGLContextPool', () => {
  beforeEach(() => {
    webGLContextPool.reset();
  });

  describe('initial state', () => {
    it('starts with zero active contexts', () => {
      expect(webGLContextPool.activeCount).toBe(0);
    });

    it('reports max contexts as 8', () => {
      expect(webGLContextPool.maxContexts).toBe(8);
    });

    it('canAcquire returns true when empty', () => {
      expect(webGLContextPool.canAcquire()).toBe(true);
    });
  });

  describe('acquire()', () => {
    it('returns null when canvas getContext returns null (no WebGL support)', () => {
      const canvas = mockCanvas('no-gl');
      const result = webGLContextPool.acquire(canvas);

      expect(result).toBeNull();
      expect(webGLContextPool.activeCount).toBe(0);
      expect(webGLContextPool.isTracked(canvas)).toBe(false);
    });

    it('returns the GL context and tracks the canvas when getContext succeeds', () => {
      const { canvas, gl } = mockCanvasWithGL('term-1');
      const result = webGLContextPool.acquire(canvas);

      expect(result).toBe(gl);
      expect(webGLContextPool.activeCount).toBe(1);
      expect(webGLContextPool.isTracked(canvas)).toBe(true);
    });

    it('is idempotent for the same canvas', () => {
      const { canvas } = mockCanvasWithGL('term-1');
      webGLContextPool.acquire(canvas);
      webGLContextPool.acquire(canvas);

      expect(webGLContextPool.activeCount).toBe(1);
    });

    it('tracks multiple different canvases', () => {
      const { canvas: c1 } = mockCanvasWithGL('term-1');
      const { canvas: c2 } = mockCanvasWithGL('term-2');
      webGLContextPool.acquire(c1);
      webGLContextPool.acquire(c2);

      expect(webGLContextPool.activeCount).toBe(2);
      expect(webGLContextPool.isTracked(c1)).toBe(true);
      expect(webGLContextPool.isTracked(c2)).toBe(true);
    });

    it('refuses acquisition when pool is at max capacity', () => {
      // Fill up the pool
      for (let i = 0; i < webGLContextPool.maxContexts; i++) {
        const { canvas } = mockCanvasWithGL(`term-${i}`);
        webGLContextPool.acquire(canvas);
      }
      expect(webGLContextPool.activeCount).toBe(webGLContextPool.maxContexts);
      expect(webGLContextPool.canAcquire()).toBe(false);

      // Next acquire should return null
      const { canvas: overflow } = mockCanvasWithGL('term-overflow');
      const result = webGLContextPool.acquire(overflow);

      expect(result).toBeNull();
      expect(webGLContextPool.isTracked(overflow)).toBe(false);
      expect(webGLContextPool.activeCount).toBe(webGLContextPool.maxContexts);
    });
  });

  describe('release()', () => {
    it('decrements active count', () => {
      const { canvas } = mockCanvasWithGL('term-1');
      webGLContextPool.acquire(canvas);
      expect(webGLContextPool.activeCount).toBe(1);

      webGLContextPool.release(canvas);

      expect(webGLContextPool.activeCount).toBe(0);
      expect(webGLContextPool.isTracked(canvas)).toBe(false);
    });

    it('is a no-op for untracked canvases', () => {
      const canvas = mockCanvas('unknown');
      webGLContextPool.release(canvas);
      expect(webGLContextPool.activeCount).toBe(0);
    });

    it('allows re-acquisition after release', () => {
      // Fill pool
      const canvases: HTMLCanvasElement[] = [];
      for (let i = 0; i < webGLContextPool.maxContexts; i++) {
        const { canvas } = mockCanvasWithGL(`term-${i}`);
        webGLContextPool.acquire(canvas);
        canvases.push(canvas);
      }
      expect(webGLContextPool.canAcquire()).toBe(false);

      // Release one
      webGLContextPool.release(canvases[0]);
      expect(webGLContextPool.canAcquire()).toBe(true);

      // New canvas can now acquire
      const { canvas: newCanvas } = mockCanvasWithGL('term-new');
      const result = webGLContextPool.acquire(newCanvas);
      expect(result).not.toBeNull();
      expect(webGLContextPool.activeCount).toBe(webGLContextPool.maxContexts);
    });
  });

  describe('notifyContextLost()', () => {
    it('removes canvas from tracking', () => {
      const { canvas } = mockCanvasWithGL('term-1');
      webGLContextPool.acquire(canvas);
      expect(webGLContextPool.activeCount).toBe(1);

      webGLContextPool.notifyContextLost(canvas);

      expect(webGLContextPool.activeCount).toBe(0);
      expect(webGLContextPool.isTracked(canvas)).toBe(false);
    });

    it('is a no-op for untracked canvases', () => {
      const canvas = mockCanvas('unknown');
      webGLContextPool.notifyContextLost(canvas);
      expect(webGLContextPool.activeCount).toBe(0);
    });

    it('frees a pool slot for other canvases', () => {
      // Fill pool
      const canvases: HTMLCanvasElement[] = [];
      for (let i = 0; i < webGLContextPool.maxContexts; i++) {
        const { canvas } = mockCanvasWithGL(`term-${i}`);
        webGLContextPool.acquire(canvas);
        canvases.push(canvas);
      }

      // Context lost on one
      webGLContextPool.notifyContextLost(canvases[3]);
      expect(webGLContextPool.canAcquire()).toBe(true);

      // New canvas can acquire
      const { canvas: newCanvas } = mockCanvasWithGL('term-new');
      expect(webGLContextPool.acquire(newCanvas)).not.toBeNull();
    });
  });

  describe('isTracked()', () => {
    it('returns false for never-acquired canvas', () => {
      const canvas = mockCanvas('unknown');
      expect(webGLContextPool.isTracked(canvas)).toBe(false);
    });

    it('returns true for acquired canvas', () => {
      const { canvas } = mockCanvasWithGL('term-1');
      webGLContextPool.acquire(canvas);
      expect(webGLContextPool.isTracked(canvas)).toBe(true);
    });

    it('returns false after release', () => {
      const { canvas } = mockCanvasWithGL('term-1');
      webGLContextPool.acquire(canvas);
      webGLContextPool.release(canvas);
      expect(webGLContextPool.isTracked(canvas)).toBe(false);
    });
  });

  describe('reset()', () => {
    it('clears all tracked contexts', () => {
      const { canvas: c1 } = mockCanvasWithGL('term-1');
      const { canvas: c2 } = mockCanvasWithGL('term-2');
      webGLContextPool.acquire(c1);
      webGLContextPool.acquire(c2);
      expect(webGLContextPool.activeCount).toBe(2);

      webGLContextPool.reset();

      expect(webGLContextPool.activeCount).toBe(0);
      expect(webGLContextPool.isTracked(c1)).toBe(false);
      expect(webGLContextPool.isTracked(c2)).toBe(false);
    });
  });
});
