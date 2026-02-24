import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock @tauri-apps/api modules (required for TerminalRenderer import)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Bug #312: No WebGL context limit — creating 25+ terminals exhausts browser
// WebGL contexts with no fallback, recovery, or context release for hidden tabs.

/**
 * Read the TerminalRenderer source to extract the WebGL context creation pattern.
 *
 * The constructor does:
 *   const gl = this.canvas.getContext('webgl2', { alpha: false, antialias: false });
 *   if (gl) { this.webglRenderer = new WebGLRenderer(gl, ...); this.useWebGL = true; }
 *
 * Problems:
 * 1. Every TerminalRenderer eagerly creates a WebGL2 context, even for hidden terminals.
 * 2. Browsers limit WebGL contexts to ~8-16 total. After that, getContext returns null.
 * 3. There is no event listener for 'webglcontextlost' to detect when a context is
 *    reclaimed by the browser.
 * 4. When a terminal is hidden (releaseCanvasResources), the WebGL context is NOT
 *    released — only canvas dimensions are shrunk to 1x1.
 */

// We test the TerminalRenderer source code properties directly rather than
// instantiating it (which requires a real DOM with canvas context support).

import { readFileSync } from 'fs';
import { resolve } from 'path';

const rendererSource = readFileSync(
  resolve(__dirname, 'TerminalRenderer.ts'),
  'utf-8',
);

describe('TerminalRenderer WebGL context management (Bug #312)', () => {
  it('should eagerly request webgl2 context for every instance (current broken behavior)', () => {
    // The constructor unconditionally calls getContext('webgl2') for every
    // TerminalRenderer. With 25 terminals, this exhausts the browser limit.
    const webgl2Calls = rendererSource.match(/getContext\(['"]webgl2['"]/g);
    expect(webgl2Calls).not.toBeNull();
    expect(webgl2Calls!.length).toBeGreaterThanOrEqual(1);

    // Verify there is NO check for total active WebGL context count before creating one.
    // A proper fix would track active contexts and skip WebGL when limit is near.
    const contextCountCheck = /activeWebGLContexts|webglContextCount|contextLimit|maxContexts/i;
    expect(rendererSource).not.toMatch(contextCountCheck);
  });

  it('should have no webglcontextlost event handler (current broken behavior)', () => {
    // Browsers fire 'webglcontextlost' when they reclaim a WebGL context.
    // Without this handler, the terminal goes blank with no recovery path.
    // The fix should listen for this event and fall back to Canvas2D.
    expect(rendererSource).not.toContain('webglcontextlost');
  });

  it('should not release WebGL context when terminal is hidden (current broken behavior)', () => {
    // releaseCanvasResources() shrinks the canvas to 1x1 but does NOT call
    // gl.getExtension('WEBGL_lose_context')?.loseContext() to free the context.
    // Hidden terminals continue holding their WebGL context, blocking visible ones.
    const releaseMethod = rendererSource.match(
      /releaseCanvasResources\(\)[^}]*\{([^}]+(?:\{[^}]*\}[^}]*)*)\}/s,
    );
    expect(releaseMethod).not.toBeNull();
    const releaseBody = releaseMethod![1];

    // The fix should call loseContext() or dispose the WebGLRenderer here.
    expect(releaseBody).not.toContain('loseContext');
    expect(releaseBody).not.toContain('dispose');
  });

  it('should not have a Canvas2D fallback triggered by context exhaustion (current broken behavior)', () => {
    // When getContext('webgl2') returns null (contexts exhausted), the code
    // does fall back to Canvas2D — but only on the first constructor call that
    // gets null. It doesn't proactively detect exhaustion or release idle contexts.
    //
    // The fix should track active context count globally and avoid WebGL when
    // approaching the browser limit, or release contexts for background terminals.
    const hasContextPool = /contextPool|recycleContext|releaseWebGL|webglContextLimit/i;
    expect(rendererSource).not.toMatch(hasContextPool);
  });

  it('should not track visibility state for context management (current broken behavior)', () => {
    // There is no mechanism to release a WebGL context when a terminal tab
    // becomes hidden and re-create it when the tab becomes visible.
    // restoreCanvasResources() does NOT recreate the WebGL context.
    const restoreMethod = rendererSource.match(
      /restoreCanvasResources\(\)[^}]*\{([^}]+(?:\{[^}]*\}[^}]*)*)\}/s,
    );
    expect(restoreMethod).not.toBeNull();
    const restoreBody = restoreMethod![1];

    // The fix should recreate WebGL context here (or use a context pool).
    expect(restoreBody).not.toContain('getContext');
    expect(restoreBody).not.toContain('WebGLRenderer');
  });
});
