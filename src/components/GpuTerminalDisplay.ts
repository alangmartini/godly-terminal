import { gpuRendererService } from '../services/gpu-renderer-service';

/**
 * GPU-rendered terminal display component.
 *
 * Replaces the Canvas2D/WebGL rendering path with an <img> element
 * that displays PNG frames rendered by the Rust-side GPU renderer.
 * Mouse events (selection, URL hover, scrollbar) remain handled by
 * the existing TerminalPane code — this component is purely for
 * painting the grid.
 */
export class GpuTerminalDisplay {
  private container: HTMLElement;
  private img: HTMLImageElement;
  private terminalId: string;
  private renderPending = false;
  private rafId: number | null = null;

  constructor(container: HTMLElement, terminalId: string) {
    this.container = container;
    this.terminalId = terminalId;

    this.img = document.createElement('img');
    this.img.style.cssText =
      'display:block;width:100%;height:100%;' +
      'object-fit:contain;image-rendering:pixelated;pointer-events:none;';
    this.container.appendChild(this.img);
  }

  /**
   * Request a new frame from the GPU renderer.
   * Debounced via requestAnimationFrame to cap at ~60fps.
   */
  requestFrame(): void {
    if (this.renderPending) return;
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
      const dataUrl = await gpuRendererService.renderTerminal(this.terminalId);
      this.img.src = dataUrl;
    } catch (err) {
      console.error('[GpuTerminalDisplay] render failed:', err);
    } finally {
      this.renderPending = false;
    }
  }

  dispose(): void {
    if (this.rafId !== null) {
      cancelAnimationFrame(this.rafId);
      this.rafId = null;
    }
    this.img.remove();
  }
}
