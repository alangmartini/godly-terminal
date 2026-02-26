import { invoke } from '@tauri-apps/api/core';

/**
 * Service for interacting with the Rust-side GPU terminal renderer.
 *
 * The GPU renderer lives entirely in the Tauri backend — it renders
 * terminal grids using wgpu. This service provides the frontend API
 * to check availability and request rendered frames as raw RGBA pixels.
 */
class GpuRendererService {
  private _available: boolean | null = null;

  /**
   * Check if the GPU renderer backend is available.
   * Caches the result after first check.
   */
  async isAvailable(): Promise<boolean> {
    if (this._available !== null) return this._available;
    try {
      this._available = await invoke<boolean>('gpu_renderer_available');
    } catch {
      this._available = false;
    }
    return this._available;
  }

  /**
   * Render a terminal and return raw RGBA bytes.
   * Format: [width: u32 LE][height: u32 LE][rgba_pixels...]
   */
  async renderTerminalRaw(terminalId: string): Promise<ArrayBuffer> {
    // DPR scaling improves text quality but makes frames 2-3x larger,
    // causing typing lag with the current offscreen GPU -> CPU readback pipeline.
    // Cap at 1.0 until we optimize (surface rendering or async double-buffer).
    const url = `http://gpuframe.localhost/render/${terminalId}?format=raw&dpr=1`;
    const response = await fetch(url);
    if (!response.ok) {
      throw new Error(`GPU render failed: ${response.status} ${response.statusText}`);
    }
    return response.arrayBuffer();
  }

  /**
   * Render a terminal using the GPU renderer.
   * Returns a data URL suitable for an <img> src.
   */
  async renderTerminal(terminalId: string): Promise<string> {
    const base64Png = await invoke<string>('render_terminal_gpu', { terminalId });
    return `data:image/png;base64,${base64Png}`;
  }

  /**
   * Get the custom protocol URL for a terminal frame.
   * This is faster than the invoke path because it avoids base64 encoding.
   */
  getFrameUrl(terminalId: string): string {
    return `http://gpuframe.localhost/render/${terminalId}`;
  }
}

export const gpuRendererService = new GpuRendererService();
