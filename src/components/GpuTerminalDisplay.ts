import { gpuRendererService } from '../services/gpu-renderer-service';

/**
 * GPU-rendered terminal display component.
 *
 * Replaces the Canvas2D/WebGL rendering path with a <canvas> element
 * that displays raw RGBA frames rendered by the Rust-side GPU renderer
 * via putImageData() for zero-copy display.
 *
 * Mouse events (selection, URL hover, scrollbar) remain handled by
 * the existing TerminalRenderer overlay — this component is purely for
 * painting the grid.
 */
export class GpuTerminalDisplay {
  private container: HTMLElement;
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private terminalId: string;
  private renderPending = false;
  private rafId: number | null = null;
  private disposed = false;

  constructor(container: HTMLElement, terminalId: string) {
    this.container = container;
    this.terminalId = terminalId;

    this.canvas = document.createElement('canvas');
    this.canvas.style.cssText = 'display:block;width:100%;height:100%;pointer-events:none;';
    this.ctx = this.canvas.getContext('2d', { alpha: false })!;
    this.container.appendChild(this.canvas);
  }

  /**
   * Request a new frame from the GPU renderer.
   * Debounced via requestAnimationFrame to cap at ~60fps.
   */
  requestFrame(): void {
    if (this.renderPending || this.disposed) return;
    this.renderPending = true;
    if (this.rafId === null) {
      this.rafId = requestAnimationFrame(() => {
        this.rafId = null;
        this.fetchAndDisplay();
      });
    }
  }

  private async fetchAndDisplay(): Promise<void> {
    try {
      const rawBytes = await gpuRendererService.renderTerminalRaw(this.terminalId);
      if (this.disposed) return;

      const view = new DataView(rawBytes);
      const width = view.getUint32(0, true);  // little-endian
      const height = view.getUint32(4, true);

      if (width === 0 || height === 0) return;

      // Resize canvas if needed
      if (this.canvas.width !== width || this.canvas.height !== height) {
        this.canvas.width = width;
        this.canvas.height = height;
      }

      const pixels = new Uint8ClampedArray(rawBytes, 8);
      const imageData = new ImageData(pixels, width, height);
      this.ctx.putImageData(imageData, 0, 0);
    } catch (err) {
      console.error('[GpuTerminalDisplay] render failed:', err);
    } finally {
      this.renderPending = false;
    }
  }

  dispose(): void {
    this.disposed = true;
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    this.canvas.remove();
  }
}
