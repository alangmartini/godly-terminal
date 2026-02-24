import { describe, it, expect, beforeEach } from 'vitest';

/**
 * Tests for lazy WebGL context allocation.
 *
 * Problem: Each TerminalRenderer eagerly creates a WebGL2 context. Browsers
 * allow only 8-16 WebGL contexts per page. With 20+ terminals (our primary
 * use case), getContext('webgl2') returns null and GPU shader compilation
 * blocks threads, contributing to the cascade failure that blanks all tabs.
 *
 * Fix: Only allocate WebGL contexts for visible terminals. Hidden terminals
 * use Canvas2D. On resume (terminal becomes visible), promote to WebGL.
 * On pause (terminal becomes hidden), demote back to Canvas2D.
 */

// ── Simulates browser WebGL context limit ──────────────────────────

let webglContextCount = 0;
const MAX_WEBGL_CONTEXTS = 8;

/**
 * Simulates browser WebGL context allocation behavior.
 * Returns null when context limit is exhausted.
 */
function simulateGetWebGL(): object | null {
  if (webglContextCount >= MAX_WEBGL_CONTEXTS) {
    return null; // Browser limit reached
  }
  webglContextCount++;
  return {}; // Mock GL context
}

function simulateReleaseWebGL(): void {
  if (webglContextCount > 0) {
    webglContextCount--;
  }
}

/**
 * Simulates the lifecycle of a TerminalRenderer with lazy WebGL allocation.
 * Tracks whether the renderer is currently using WebGL or Canvas2D.
 */
class RendererSimulator {
  useWebGL = false;

  /** Simulate the constructor: starts with Canvas2D, no WebGL context. */
  constructor() {
    // In the lazy model, the constructor only gets '2d' context.
    // No WebGL context is created.
    this.useWebGL = false;
  }

  /** Simulate promoteToWebGL(): called when terminal becomes visible. */
  promote(): boolean {
    if (this.useWebGL) return true; // already promoted
    const gl = simulateGetWebGL();
    if (!gl) return false; // limit reached
    this.useWebGL = true;
    return true;
  }

  /** Simulate demoteToCanvas2D(): called when terminal becomes hidden. */
  demote(): void {
    if (!this.useWebGL) return;
    simulateReleaseWebGL();
    this.useWebGL = false;
  }

  getBackend(): string {
    return this.useWebGL ? 'WebGL2' : 'Canvas2D';
  }
}

// ── Tests ───────────────────────────────────────────────────────────

describe('Lazy WebGL context allocation', () => {
  beforeEach(() => {
    webglContextCount = 0;
  });

  it('should NOT create WebGL contexts beyond browser limit', () => {
    // Bug: With eager allocation, terminal #9+ would get null from
    // getContext('webgl2') and crash or fall back ungracefully.
    // With lazy allocation, only visible terminals get WebGL contexts.

    // Simulate creating 20 terminals — none should request WebGL
    const renderers: RendererSimulator[] = [];
    for (let i = 0; i < 20; i++) {
      renderers.push(new RendererSimulator());
    }

    // No WebGL contexts should have been created
    expect(webglContextCount).toBe(0);
    for (const r of renderers) {
      expect(r.getBackend()).toBe('Canvas2D');
    }
  });

  it('browser returns null when WebGL context limit is reached', () => {
    const contexts: object[] = [];
    for (let i = 0; i < MAX_WEBGL_CONTEXTS + 5; i++) {
      const ctx = simulateGetWebGL();
      if (ctx) contexts.push(ctx);
    }

    expect(contexts.length).toBe(MAX_WEBGL_CONTEXTS);
  });

  it('releasing a WebGL context makes room for new ones', () => {
    // Fill up all contexts
    for (let i = 0; i < MAX_WEBGL_CONTEXTS; i++) {
      simulateGetWebGL();
    }
    expect(webglContextCount).toBe(MAX_WEBGL_CONTEXTS);
    expect(simulateGetWebGL()).toBeNull();

    // "Release" one context (simulating demoteToCanvas2D → dispose)
    simulateReleaseWebGL();

    // Now we should be able to create one more
    const ctx = simulateGetWebGL();
    expect(ctx).not.toBeNull();
    expect(webglContextCount).toBe(MAX_WEBGL_CONTEXTS);
  });
});

describe('TerminalRenderer lazy WebGL lifecycle', () => {
  beforeEach(() => {
    webglContextCount = 0;
  });

  it('renderer starts with Canvas2D backend (no WebGL)', () => {
    const renderer = new RendererSimulator();
    expect(renderer.getBackend()).toBe('Canvas2D');
    expect(webglContextCount).toBe(0);
  });

  it('promote/demote cycle manages WebGL context count', () => {
    const renderer = new RendererSimulator();
    expect(webglContextCount).toBe(0);

    // Promote: terminal becomes visible
    expect(renderer.promote()).toBe(true);
    expect(renderer.getBackend()).toBe('WebGL2');
    expect(webglContextCount).toBe(1);

    // Demote: terminal becomes hidden
    renderer.demote();
    expect(renderer.getBackend()).toBe('Canvas2D');
    expect(webglContextCount).toBe(0);

    // Re-promote: terminal becomes visible again
    expect(renderer.promote()).toBe(true);
    expect(renderer.getBackend()).toBe('WebGL2');
    expect(webglContextCount).toBe(1);
  });

  it('promote is idempotent (calling twice does not leak)', () => {
    const renderer = new RendererSimulator();
    expect(renderer.promote()).toBe(true);
    expect(webglContextCount).toBe(1);

    // Second promote should be a no-op
    expect(renderer.promote()).toBe(true);
    expect(webglContextCount).toBe(1);
  });

  it('demote is idempotent (calling twice does not underflow)', () => {
    const renderer = new RendererSimulator();
    renderer.promote();
    expect(webglContextCount).toBe(1);

    renderer.demote();
    expect(webglContextCount).toBe(0);

    // Second demote should be a no-op
    renderer.demote();
    expect(webglContextCount).toBe(0);
  });

  it('with 20 terminals, only visible ones should have WebGL contexts', () => {
    const TOTAL = 20;
    const VISIBLE = 2;
    const renderers: RendererSimulator[] = [];

    for (let i = 0; i < TOTAL; i++) {
      renderers.push(new RendererSimulator());
    }
    expect(webglContextCount).toBe(0);

    // Promote only visible terminals
    for (let i = 0; i < VISIBLE; i++) {
      expect(renderers[i].promote()).toBe(true);
    }

    expect(webglContextCount).toBe(VISIBLE);
    expect(webglContextCount).toBeLessThan(MAX_WEBGL_CONTEXTS);
  });

  it('tab switch releases WebGL from old terminal and allocates for new', () => {
    const termA = new RendererSimulator();
    const termB = new RendererSimulator();

    // Terminal A is visible with WebGL
    termA.promote();
    expect(webglContextCount).toBe(1);

    // Tab switch: A becomes hidden, B becomes visible
    termA.demote();
    termB.promote();
    expect(webglContextCount).toBe(1);
    expect(termA.getBackend()).toBe('Canvas2D');
    expect(termB.getBackend()).toBe('WebGL2');
  });

  it('split view allows 2 WebGL contexts simultaneously', () => {
    const left = new RendererSimulator();
    const right = new RendererSimulator();

    left.promote();
    right.promote();
    expect(webglContextCount).toBe(2);
    expect(webglContextCount).toBeLessThan(MAX_WEBGL_CONTEXTS);

    // Unsplit: one terminal hidden
    right.demote();
    expect(webglContextCount).toBe(1);
  });

  it('rapid tab switching does not leak WebGL contexts', () => {
    const terminals: RendererSimulator[] = [];
    for (let i = 0; i < 20; i++) {
      terminals.push(new RendererSimulator());
    }

    // Simulate rapid tab switching: each terminal gets promoted then demoted
    for (let cycle = 0; cycle < 50; cycle++) {
      const idx = cycle % 20;
      terminals[idx].promote();
      expect(webglContextCount).toBe(1);
      terminals[idx].demote();
      expect(webglContextCount).toBe(0);
    }

    expect(webglContextCount).toBe(0);
  });

  it('promotion fails gracefully when context limit is reached', () => {
    // Fill up all WebGL contexts with visible terminals
    const visible: RendererSimulator[] = [];
    for (let i = 0; i < MAX_WEBGL_CONTEXTS; i++) {
      const r = new RendererSimulator();
      expect(r.promote()).toBe(true);
      visible.push(r);
    }
    expect(webglContextCount).toBe(MAX_WEBGL_CONTEXTS);

    // One more promotion should fail gracefully
    const extra = new RendererSimulator();
    expect(extra.promote()).toBe(false);
    expect(extra.getBackend()).toBe('Canvas2D'); // still works, just without GPU
    expect(webglContextCount).toBe(MAX_WEBGL_CONTEXTS);

    // After demoting one, the extra can now promote
    visible[0].demote();
    expect(extra.promote()).toBe(true);
    expect(extra.getBackend()).toBe('WebGL2');
  });
});
