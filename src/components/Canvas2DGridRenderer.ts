/**
 * Canvas2D-based terminal grid renderer.
 *
 * Replaces the GPU offscreen readback pipeline (GpuTerminalDisplay) with
 * browser-native Canvas2D rendering. Uses the same RichGridData already
 * fetched for the overlay, eliminating the separate gpuframe:// fetch.
 *
 * Benefits:
 * - Native DPR scaling (canvas bitmap = CSS size * devicePixelRatio)
 * - Browser ClearType/DirectWrite font rendering (matches Windows Terminal)
 * - Zero transfer latency (no GPU→CPU readback, no HTTP, no putImageData)
 * - Synchronous rendering from in-memory snapshot data
 */

import type { RichGridData, RichGridCell } from './TerminalRenderer';
import type { TerminalTheme } from '../themes/types';
import { themeStore } from '../state/theme-store';
import { terminalSettingsStore } from '../state/terminal-settings-store';

export class Canvas2DGridRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private theme: TerminalTheme;

  // Font metrics (in device pixels, scaled by DPR)
  private fontFamily = 'Cascadia Code, Consolas, monospace';
  private fontSize = terminalSettingsStore.getFontSize();
  private cellWidth = 0;
  private cellHeight = 0;
  private baseline = 0; // Y offset from cell top to text baseline
  private dpr = 1;

  // Cursor blink state
  private cursorVisible = true;
  private cursorBlinkTimer: ReturnType<typeof setInterval> | null = null;
  private focused = true; // Whether this pane has keyboard focus

  // Repaint callback (called on cursor blink to trigger re-render)
  private onRepaintNeeded: (() => void) | null = null;

  constructor(container: HTMLElement) {
    this.canvas = document.createElement('canvas');
    this.canvas.style.cssText = 'display:block;width:100%;height:100%;pointer-events:none;';
    // Opaque canvas (alpha: false) is faster — no compositing with page background
    this.ctx = this.canvas.getContext('2d', { alpha: false })!;
    container.appendChild(this.canvas);

    this.theme = themeStore.getTerminalTheme();
    this.dpr = window.devicePixelRatio || 1;
    this.measureFont();
    this.startCursorBlink();
  }

  /** Set a callback for when the renderer needs a repaint (e.g. cursor blink). */
  setOnRepaintNeeded(cb: () => void): void {
    this.onRepaintNeeded = cb;
  }

  /** Update the terminal theme. Does not trigger repaint — caller should re-render. */
  setTheme(theme: TerminalTheme): void {
    this.theme = theme;
  }

  /**
   * Set whether this pane has keyboard focus. Unfocused panes render an
   * outline cursor and stop blinking; focused panes render a solid block
   * cursor with blinking.
   */
  setFocused(value: boolean): void {
    if (value === this.focused) return;
    this.focused = value;
    if (value) {
      this.startCursorBlink();
    } else {
      this.stopCursorBlink();
      this.cursorVisible = true; // Keep cursor visible (as outline) when unfocused
      if (this.onRepaintNeeded) this.onRepaintNeeded();
    }
  }

  /** Update font size and re-measure. Does not trigger repaint. */
  setFontSize(size: number): void {
    if (size === this.fontSize) return;
    this.fontSize = size;
    this.measureFont();
  }

  /** Get the cell dimensions in device pixels. */
  getCellSize(): { width: number; height: number } {
    return { width: this.cellWidth, height: this.cellHeight };
  }

  /**
   * Recalculate canvas size to match its CSS layout size.
   * Returns true if the size changed.
   */
  updateSize(): boolean {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const newW = Math.floor(rect.width * dpr);
    const newH = Math.floor(rect.height * dpr);

    if (this.canvas.width === newW && this.canvas.height === newH && this.dpr === dpr) {
      return false;
    }

    this.dpr = dpr;
    this.canvas.width = newW;
    this.canvas.height = newH;
    this.measureFont();
    return true;
  }

  /**
   * Optimistic scroll: shift existing canvas content by deltaLines immediately.
   * Gives instant visual feedback while the real snapshot IPC is in flight.
   * Positive delta = scrolling up (into history), content moves down on screen.
   */
  shiftCanvas(deltaLines: number): void {
    if (deltaLines === 0 || this.cellHeight === 0) return;

    const ctx = this.ctx;
    const w = this.canvas.width;
    const h = this.canvas.height;
    const shiftPx = Math.round(deltaLines * this.cellHeight);

    if (Math.abs(shiftPx) >= h) {
      ctx.fillStyle = this.theme.background;
      ctx.fillRect(0, 0, w, h);
      return;
    }

    if (shiftPx > 0) {
      // Scrolling up: content moves down, new rows appear at top
      ctx.drawImage(this.canvas, 0, 0, w, h - shiftPx, 0, shiftPx, w, h - shiftPx);
      ctx.fillStyle = this.theme.background;
      ctx.fillRect(0, 0, w, shiftPx);
    } else {
      // Scrolling down: content moves up, new rows appear at bottom
      const absShift = -shiftPx;
      ctx.drawImage(this.canvas, 0, absShift, w, h - absShift, 0, 0, w, h - absShift);
      ctx.fillStyle = this.theme.background;
      ctx.fillRect(0, h - absShift, w, absShift);
    }
  }

  /**
   * Render a terminal grid snapshot to the canvas.
   * This is synchronous and fast — no IPC, no async pipeline.
   */
  render(snapshot: RichGridData): void {
    const ctx = this.ctx;
    const { cellWidth, cellHeight, baseline, dpr } = this;
    const theme = this.theme;

    if (cellWidth === 0 || cellHeight === 0) return;

    // 1. Fill entire canvas with default background
    ctx.fillStyle = theme.background;
    ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

    const rows = snapshot.rows;
    const numCols = snapshot.dimensions.cols;

    // 2. Non-default cell backgrounds (horizontal run-length merged)
    for (let row = 0; row < rows.length; row++) {
      const gridRow = rows[row];
      const y = row * cellHeight;
      let runStart = 0;
      let runColor: string | null = null;
      let runLength = 0;

      for (let col = 0; col < gridRow.cells.length && col < numCols; col++) {
        const cell = gridRow.cells[col];
        const bg = this.resolveBg(cell);

        if (bg === runColor) {
          runLength++;
        } else {
          if (runColor !== null) {
            ctx.fillStyle = runColor;
            ctx.fillRect(runStart * cellWidth, y, runLength * cellWidth, cellHeight);
          }
          if (bg !== null) {
            runStart = col;
            runColor = bg;
            runLength = 1;
          } else {
            runColor = null;
            runLength = 0;
          }
        }
      }
      if (runColor !== null) {
        ctx.fillStyle = runColor;
        ctx.fillRect(runStart * cellWidth, y, runLength * cellWidth, cellHeight);
      }
    }

    // 3. Cursor overlay (behind text so text is visible on cursor)
    if (!snapshot.cursor_hidden && this.cursorVisible) {
      const cr = snapshot.cursor.row;
      const cc = snapshot.cursor.col;
      if (cr >= 0 && cr < rows.length && cc >= 0 && cc < numCols) {
        const cx = cc * cellWidth;
        const cy = cr * cellHeight;
        if (this.focused) {
          // Solid block cursor for focused pane
          ctx.fillStyle = theme.cursor;
          ctx.fillRect(cx, cy, cellWidth, cellHeight);
        } else {
          // Outline (hollow) cursor for unfocused pane
          const lineWidth = Math.max(1, dpr);
          ctx.strokeStyle = theme.cursor;
          ctx.lineWidth = lineWidth;
          const half = lineWidth / 2;
          ctx.strokeRect(cx + half, cy + half, cellWidth - lineWidth, cellHeight - lineWidth);
        }
      }
    }

    // 4. Text rendering — batch by font variant to minimize ctx.font changes
    // Collect cells by style key, then render each batch with one font set
    let currentFont = '';
    for (let row = 0; row < rows.length; row++) {
      const gridRow = rows[row];
      const y = row * cellHeight + baseline;
      for (let col = 0; col < gridRow.cells.length && col < numCols; col++) {
        const cell = gridRow.cells[col];
        if (!cell.content || cell.content === ' ' || cell.wide_continuation) continue;

        // Resolve foreground color
        let fg = this.resolveFg(cell);

        // On cursor position, use cursorAccent for text color (only on solid/focused cursor)
        if (this.focused && !snapshot.cursor_hidden && this.cursorVisible &&
            row === snapshot.cursor.row && col === snapshot.cursor.col) {
          fg = theme.cursorAccent;
        }

        // Dim: reduce to 50% opacity via globalAlpha
        if (cell.dim) {
          ctx.globalAlpha = 0.5;
        }

        // Set font (only when style changes)
        const font = this.fontString(cell.bold, cell.italic);
        if (font !== currentFont) {
          ctx.font = font;
          currentFont = font;
        }

        ctx.fillStyle = fg;
        const x = col * cellWidth;
        ctx.fillText(cell.content, x, y);

        if (cell.dim) {
          ctx.globalAlpha = 1.0;
        }

        // Underline
        if (cell.underline) {
          const uly = row * cellHeight + cellHeight - Math.max(1, dpr);
          ctx.fillRect(x, uly, cellWidth, Math.max(1, dpr));
        }
      }
    }
  }

  /** Release canvas memory (called when terminal is hidden). */
  releaseResources(): void {
    this.canvas.width = 1;
    this.canvas.height = 1;
    this.stopCursorBlink();
  }

  /** Restore canvas resources after release (called when terminal becomes visible). */
  restoreResources(): void {
    if (this.focused && !this.cursorBlinkTimer) {
      this.startCursorBlink();
    }
  }

  dispose(): void {
    this.stopCursorBlink();
    this.canvas.width = 1;
    this.canvas.height = 1;
    this.canvas.remove();
  }

  // ---- Private: Font measurement ----

  private measureFont(): void {
    const dpr = this.dpr;
    // Round to integer pixel size for clean font hinting
    const scaledSize = Math.round(this.fontSize * dpr);
    const font = `${scaledSize}px ${this.fontFamily}`;
    this.ctx.font = font;
    const metrics = this.ctx.measureText('M');
    this.cellWidth = Math.ceil(metrics.width);
    // Line height ≈ fontSize * 1.2
    this.cellHeight = Math.ceil(scaledSize * 1.2);
    // Baseline: use font metrics if available, else approximate
    this.baseline = metrics.fontBoundingBoxAscent !== undefined
      ? Math.ceil(metrics.fontBoundingBoxAscent)
      : Math.ceil(scaledSize * 0.85);
  }

  private fontString(bold: boolean, italic: boolean): string {
    const scaledSize = Math.round(this.fontSize * this.dpr);
    let style = '';
    if (bold) style += 'bold ';
    if (italic) style += 'italic ';
    return `${style}${scaledSize}px ${this.fontFamily}`;
  }

  // ---- Private: Color resolution ----

  private resolveFg(cell: RichGridCell): string {
    if (cell.inverse) {
      return cell.bg === 'default' ? this.theme.background : cell.bg;
    }
    return cell.fg === 'default' ? this.theme.foreground : cell.fg;
  }

  private resolveBg(cell: RichGridCell): string | null {
    let bg: string;
    if (cell.inverse) {
      bg = cell.fg === 'default' ? this.theme.foreground : cell.fg;
    } else {
      if (cell.bg === 'default') return null; // skip — already filled with default bg
      bg = cell.bg;
    }
    return bg;
  }

  // ---- Private: Cursor blink ----

  private startCursorBlink(): void {
    this.cursorBlinkTimer = setInterval(() => {
      this.cursorVisible = !this.cursorVisible;
      if (this.onRepaintNeeded) {
        this.onRepaintNeeded();
      }
    }, 600);
  }

  private stopCursorBlink(): void {
    if (this.cursorBlinkTimer) {
      clearInterval(this.cursorBlinkTimer);
      this.cursorBlinkTimer = null;
    }
  }
}
