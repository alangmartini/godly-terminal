import { describe, it, expect, vi } from 'vitest';

// Mock @tauri-apps/api modules (required for TerminalRenderer import)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Bug #312: WebGL context exhaustion with 25+ terminals.
// Bug #337: WebGL context pool leak via demoteToCanvas2D / promoteToWebGL bypass.
//
// These tests verify the source code contains the correct patterns for
// WebGL context management, pool tracking, and resource cleanup.

import { readFileSync } from 'fs';
import { resolve } from 'path';

const rendererSource = readFileSync(
  resolve(__dirname, 'TerminalRenderer.ts'),
  'utf-8',
);

describe('TerminalRenderer WebGL context management (Bug #312, #337)', () => {
  it('should use WebGL context pool for all context acquisition', () => {
    // Both acquireWebGL() and promoteToWebGL() should go through the pool.
    expect(rendererSource).toContain('webGLContextPool.acquire');
  });

  it('should handle webglcontextlost events', () => {
    // The renderer must listen for context loss to fall back to Canvas2D.
    expect(rendererSource).toContain('webglcontextlost');
  });

  it('demoteToCanvas2D should release pool slot before replacing canvas', () => {
    // Bug #337: demoteToCanvas2D must call webGLContextPool.release()
    // BEFORE replacing the canvas, so the pool correctly tracks the old canvas.
    // Verify the release call appears between demoteToCanvas2D declaration and
    // the canvas replacement (createElement).
    const demoteStart = rendererSource.indexOf('demoteToCanvas2D(): void {');
    expect(demoteStart).toBeGreaterThan(-1);
    const releaseInDemote = rendererSource.indexOf('webGLContextPool.release(this.canvas)', demoteStart);
    expect(releaseInDemote).toBeGreaterThan(demoteStart);
    // Verify release happens before the canvas is replaced
    const createCanvasInDemote = rendererSource.indexOf("document.createElement('canvas')", demoteStart);
    expect(createCanvasInDemote).toBeGreaterThan(releaseInDemote);
  });

  it('promoteToWebGL should acquire through pool, not raw getContext', () => {
    // Bug #337: promoteToWebGL must use webGLContextPool.acquire() instead
    // of calling newCanvas.getContext('webgl2') directly.
    const promoteStart = rendererSource.indexOf('promoteToWebGL(): boolean {');
    expect(promoteStart).toBeGreaterThan(-1);
    // Find the next method (demoteToCanvas2D) to bound the search
    const promoteEnd = rendererSource.indexOf('demoteToCanvas2D(): void {', promoteStart);
    const promoteBody = rendererSource.slice(promoteStart, promoteEnd);
    expect(promoteBody).toContain('webGLContextPool.acquire');
    // Should NOT have a raw getContext('webgl2') call (that bypasses the pool)
    expect(promoteBody).not.toMatch(/\.getContext\(['"]webgl2['"]/);
  });

  it('restoreCanvasResources should try promoteToWebGL when acquireWebGL fails', () => {
    const restoreStart = rendererSource.indexOf('restoreCanvasResources() {');
    expect(restoreStart).toBeGreaterThan(-1);
    // Find next method to bound the search
    const nextMethodIdx = rendererSource.indexOf('promoteToWebGL(): boolean {', restoreStart);
    const restoreBody = rendererSource.slice(restoreStart, nextMethodIdx);
    expect(restoreBody).toContain('acquireWebGL');
    expect(restoreBody).toContain('promoteToWebGL');
  });
});
