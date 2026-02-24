/**
 * WebGL context pool: tracks active WebGL2 contexts and enforces a safe limit.
 *
 * Browsers limit WebGL contexts to ~8-16 per page. With 25+ terminals, naive
 * per-terminal context creation exhausts this limit and getContext('webgl2')
 * returns null with no recovery. This pool ensures only visible terminals hold
 * WebGL contexts; hidden terminals fall back to Canvas2D or a static snapshot.
 *
 * Usage:
 *   - Call `acquire(canvas)` when a terminal becomes visible
 *   - Call `release(canvas)` when a terminal becomes hidden
 *   - The pool tracks active contexts and refuses new ones beyond MAX_CONTEXTS
 */

/** Safe limit for concurrent WebGL contexts. Most browsers support 8-16. */
const MAX_CONTEXTS = 8;

class WebGLContextPoolImpl {
  /** Set of canvases that currently hold an active WebGL2 context. */
  private activeContexts: Set<HTMLCanvasElement> = new Set();

  /** Returns the number of currently active WebGL contexts. */
  get activeCount(): number {
    return this.activeContexts.size;
  }

  /** Returns the maximum number of concurrent contexts allowed. */
  get maxContexts(): number {
    return MAX_CONTEXTS;
  }

  /** Returns true if a new context can be acquired without exceeding the limit. */
  canAcquire(): boolean {
    return this.activeContexts.size < MAX_CONTEXTS;
  }

  /**
   * Try to acquire a WebGL2 context for the given canvas.
   * Returns the context if successful, or null if the limit is reached or
   * the browser refuses to create the context.
   */
  acquire(canvas: HTMLCanvasElement): WebGL2RenderingContext | null {
    // Already tracked — return existing context
    if (this.activeContexts.has(canvas)) {
      const existing = canvas.getContext('webgl2');
      return existing as WebGL2RenderingContext | null;
    }

    if (!this.canAcquire()) {
      console.warn(
        `[WebGLContextPool] Cannot acquire: ${this.activeContexts.size}/${MAX_CONTEXTS} contexts in use`
      );
      return null;
    }

    const gl = canvas.getContext('webgl2', { alpha: false, antialias: false });
    if (gl) {
      this.activeContexts.add(canvas);
      console.log(
        `[WebGLContextPool] Acquired context (${this.activeContexts.size}/${MAX_CONTEXTS})`
      );
    }
    return gl;
  }

  /**
   * Release a WebGL context for the given canvas.
   * The context is lost by the browser when we call loseContext() via the
   * WEBGL_lose_context extension, or simply by letting the canvas be GC'd.
   * We just untrack it here — the actual GL cleanup is the caller's responsibility.
   */
  release(canvas: HTMLCanvasElement): void {
    if (!this.activeContexts.has(canvas)) return;
    this.activeContexts.delete(canvas);
    console.log(
      `[WebGLContextPool] Released context (${this.activeContexts.size}/${MAX_CONTEXTS})`
    );
  }

  /**
   * Notify the pool that a context was lost (e.g., via webglcontextlost event).
   * Removes the canvas from tracking so a new context can be acquired later.
   */
  notifyContextLost(canvas: HTMLCanvasElement): void {
    this.activeContexts.delete(canvas);
    console.log(
      `[WebGLContextPool] Context lost notification (${this.activeContexts.size}/${MAX_CONTEXTS})`
    );
  }

  /**
   * Check whether a canvas currently holds a tracked WebGL context.
   */
  isTracked(canvas: HTMLCanvasElement): boolean {
    return this.activeContexts.has(canvas);
  }

  /** Reset the pool (for testing). */
  reset(): void {
    this.activeContexts.clear();
  }
}

/** Singleton pool instance. */
export const webGLContextPool = new WebGLContextPoolImpl();
