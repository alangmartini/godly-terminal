/**
 * Canvas2D terminal renderer.
 *
 * Paints grid snapshots from godly-vt (via Tauri IPC)
 * onto a <canvas> element. The godly-vt parser in the daemon owns the terminal
 * state; this renderer is purely a display layer.
 *
 * Design:
 * - render(snapshot) paints the full grid with per-cell colors and attributes
 * - requestAnimationFrame loop ensures at most 60fps re-renders
 * - Selection is handled via mouse events -> grid coords -> highlight overlay
 * - URL detection underlines URLs on hover, opens on Ctrl+click
 */

import { invoke } from '@tauri-apps/api/core';
import { WebGLRenderer } from './renderer/WebGLRenderer';
import { perfTracer } from '../utils/PerfTracer';
import { themeStore } from '../state/theme-store';
import type { TerminalTheme } from '../themes/types';

export type { TerminalTheme } from '../themes/types';

// ---- Types matching the Rust RichGridData ----

export interface RichGridData {
  rows: RichGridRow[];
  cursor: CursorState;
  dimensions: GridDimensions;
  alternate_screen: boolean;
  cursor_hidden: boolean;
  title: string;
  scrollback_offset: number;
  total_scrollback: number;
}

export interface RichGridRow {
  cells: RichGridCell[];
  wrapped: boolean;
}

export interface RichGridCell {
  content: string;
  fg: string;
  bg: string;
  bold: boolean;
  dim: boolean;
  italic: boolean;
  underline: boolean;
  inverse: boolean;
  wide: boolean;
  wide_continuation: boolean;
}

export interface CursorState {
  row: number;
  col: number;
}

export interface GridDimensions {
  rows: number;
  cols: number;
}

/** Differential grid snapshot: only contains rows that changed since last read. */
export interface RichGridDiff {
  dirty_rows: [number, RichGridRow][];
  cursor: CursorState;
  dimensions: GridDimensions;
  alternate_screen: boolean;
  cursor_hidden: boolean;
  title: string;
  scrollback_offset: number;
  total_scrollback: number;
  full_repaint: boolean;
}

// ---- Backward compatibility: re-export DEFAULT_THEME from builtin ----

import { TOKYO_NIGHT } from '../themes/builtin';
export const DEFAULT_THEME: TerminalTheme = TOKYO_NIGHT.terminal;

// ---- Selection state ----

interface Selection {
  startRow: number;
  startCol: number;
  endRow: number;
  endCol: number;
  active: boolean;
}

// ---- URL pattern for link detection ----

const URL_REGEX = /https?:\/\/[^\s<>'")\]]+/g;

// ---- Renderer backend info (set once on first construction) ----

let _rendererBackend: string | null = null;

/** Returns the rendering backend used by terminal panes ('WebGL2' or 'Canvas2D'). */
export function getRendererBackend(): string {
  return _rendererBackend ?? 'unknown';
}

// ---- Renderer ----

export class TerminalRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D | null = null;
  private theme: TerminalTheme;

  // WebGL
  private webglRenderer: WebGLRenderer | null = null;
  private overlayCanvas: HTMLCanvasElement | null = null;
  private overlayCtx: CanvasRenderingContext2D | null = null;
  private useWebGL = false;

  // Font metrics
  private fontFamily = 'Cascadia Code, Consolas, monospace';
  private fontSize = 13;
  private cellWidth = 0;
  private cellHeight = 0;
  private baselineOffset = 0;
  private devicePixelRatio = 1;

  // State
  private currentSnapshot: RichGridData | null = null;
  private pendingSnapshot: RichGridData | null = null;
  private renderScheduled = false;

  // Cursor blink
  private cursorVisible = true;
  private cursorBlinkInterval: ReturnType<typeof setInterval> | null = null;

  // Selection
  private selection: Selection = {
    startRow: 0,
    startCol: 0,
    endRow: 0,
    endCol: 0,
    active: false,
  };
  private isSelecting = false;

  // URL hover
  private hoveredUrl: string | null = null;
  private hoveredUrlRow = -1;
  private hoveredUrlStartCol = -1;
  private hoveredUrlEndCol = -1;

  // Scrollbar drag state
  private isDraggingScrollbar = false;
  private onDocumentMouseMove: ((e: MouseEvent) => void) | null = null;
  private onDocumentMouseUp: ((e: MouseEvent) => void) | null = null;

  // Callbacks
  private onTitleChange?: (title: string) => void;
  private onScrollCallback?: (deltaLines: number) => void;
  private onScrollToCallback?: (absoluteOffset: number) => void;

  constructor(theme?: Partial<TerminalTheme>) {
    this.theme = theme ? { ...themeStore.getTerminalTheme(), ...theme } : themeStore.getTerminalTheme();
    this.canvas = document.createElement('canvas');
    this.canvas.className = 'terminal-canvas';
    this.canvas.style.display = 'block';
    this.canvas.style.width = '100%';
    this.canvas.style.height = '100%';
    this.canvas.tabIndex = 0;
    // Prevent default context menu on right-click (we handle copy ourselves)
    this.canvas.addEventListener('contextmenu', (e) => e.preventDefault());

    // Try WebGL2 first
    const gl = this.canvas.getContext('webgl2', { alpha: false, antialias: false });
    if (gl) {
      try {
        console.log('[TerminalRenderer] WebGL2 context obtained, initializing GPU renderer...');
        this.webglRenderer = new WebGLRenderer(gl, this.fontFamily, this.fontSize, window.devicePixelRatio || 1);
        this.useWebGL = true;
        console.log('[TerminalRenderer] WebGL2 renderer initialized successfully');
        // Create overlay canvas for scrollbar and URL hover
        this.overlayCanvas = document.createElement('canvas');
        this.overlayCanvas.className = 'terminal-overlay-canvas';
        this.overlayCanvas.style.display = 'block';
        this.overlayCtx = this.overlayCanvas.getContext('2d')!;
      } catch (e) {
        console.warn('[TerminalRenderer] WebGL2 renderer init failed, falling back to Canvas2D:', e);
      }
    } else {
      console.log('[TerminalRenderer] WebGL2 not available, using Canvas2D');
    }

    if (!this.useWebGL) {
      // If WebGL was attempted (getContext('webgl2') succeeded but renderer threw),
      // the canvas is locked to WebGL and can't get a 2D context. Create a new canvas.
      let ctx2d = this.canvas.getContext('2d', { alpha: false });
      if (!ctx2d) {
        console.log('[TerminalRenderer] Canvas locked to WebGL, creating fresh canvas for 2D fallback');
        this.canvas = document.createElement('canvas');
        this.canvas.className = 'terminal-canvas';
        this.canvas.style.display = 'block';
        this.canvas.style.width = '100%';
        this.canvas.style.height = '100%';
        this.canvas.tabIndex = 0;
        this.canvas.addEventListener('contextmenu', (e) => e.preventDefault());
        ctx2d = this.canvas.getContext('2d', { alpha: false })!;
      }
      this.ctx = ctx2d;
      console.log('[TerminalRenderer] Canvas2D fallback active');
    }

    if (this.useWebGL && this.webglRenderer) {
      const metrics = this.webglRenderer.measureFont();
      this.cellWidth = metrics.cellWidth;
      this.cellHeight = metrics.cellHeight;
    } else {
      this.measureFont();
    }

    _rendererBackend = this.useWebGL ? 'WebGL2' : 'Canvas2D';

    this.setupMouseHandlers();
    this.setupWheelHandler();
    this.startCursorBlink();
  }

  /** Get the canvas element for mounting into the DOM. */
  getElement(): HTMLCanvasElement {
    return this.canvas;
  }

  /** Returns the active rendering backend name. */
  getBackend(): string {
    return this.useWebGL ? 'WebGL2' : 'Canvas2D';
  }

  /** Update the terminal theme and trigger a repaint. */
  setTheme(theme: TerminalTheme): void {
    this.theme = theme;
    this.repaint();
  }

  /** Get the current grid dimensions in rows/cols based on canvas size. */
  getGridSize(): { rows: number; cols: number } {
    if (this.cellWidth === 0 || this.cellHeight === 0) {
      return { rows: 24, cols: 80 };
    }
    const rect = this.canvas.getBoundingClientRect();
    const rows = Math.max(1, Math.floor(rect.height / (this.cellHeight / this.devicePixelRatio)));
    const cols = Math.max(1, Math.floor(rect.width / (this.cellWidth / this.devicePixelRatio)));
    return { rows, cols };
  }

  /** Set title change callback. */
  setOnTitleChange(cb: (title: string) => void) {
    this.onTitleChange = cb;
  }

  /** Set scroll callback. deltaLines > 0 = scroll up (into history), < 0 = scroll down (toward live). */
  setOnScroll(cb: (deltaLines: number) => void) {
    this.onScrollCallback = cb;
  }

  /** Set absolute scroll-to callback (used by scrollbar drag). */
  setOnScrollTo(cb: (absoluteOffset: number) => void) {
    this.onScrollToCallback = cb;
  }

  /**
   * Schedule a render with the given snapshot.
   * Uses requestAnimationFrame to avoid rendering faster than 60fps.
   */
  render(snapshot: RichGridData) {
    this.pendingSnapshot = snapshot;

    // Notify title changes
    if (snapshot.title && this.onTitleChange) {
      this.onTitleChange(snapshot.title);
    }

    if (!this.renderScheduled) {
      this.renderScheduled = true;
      perfTracer.mark('render_start');
      requestAnimationFrame(() => {
        this.renderScheduled = false;
        if (this.pendingSnapshot) {
          this.currentSnapshot = this.pendingSnapshot;
          this.pendingSnapshot = null;
          perfTracer.measure('raf_wait', 'render_start');
          this.paint();
          perfTracer.measure('paint_duration', 'paint_start');
          perfTracer.measure('keydown_to_paint', 'keydown');
          perfTracer.tick();
        }
      });
    }
  }

  /** Force an immediate re-render of the current snapshot. */
  repaint() {
    if (this.currentSnapshot) {
      this.paint();
    }
  }

  /**
   * Recalculate canvas size to match its CSS layout size.
   * Call after the container resizes. Returns true if the size changed.
   */
  updateSize(): boolean {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = window.devicePixelRatio || 1;
    const newWidth = Math.floor(rect.width * dpr);
    const newHeight = Math.floor(rect.height * dpr);

    if (this.canvas.width === newWidth && this.canvas.height === newHeight && this.devicePixelRatio === dpr) {
      return false;
    }

    this.devicePixelRatio = dpr;
    this.canvas.width = newWidth;
    this.canvas.height = newHeight;

    if (this.useWebGL && this.webglRenderer) {
      this.webglRenderer.resize(newWidth, newHeight, dpr);
      const metrics = this.webglRenderer.measureFont();
      this.cellWidth = metrics.cellWidth;
      this.cellHeight = metrics.cellHeight;
      // Resize overlay canvas
      if (this.overlayCanvas) {
        this.overlayCanvas.width = newWidth;
        this.overlayCanvas.height = newHeight;
      }
    } else {
      this.measureFont();
    }
    return true;
  }

  /** Check if there is an active text selection. */
  hasSelection(): boolean {
    return this.selection.active;
  }

  /** Get the current selection bounds (normalized so start <= end). */
  getSelection(): Selection | null {
    if (!this.selection.active) return null;
    return this.normalizeSelection(this.selection);
  }

  /** Clear the current selection. */
  clearSelection() {
    this.selection.active = false;
    this.repaint();
  }

  /** Get selected text by calling the backend. */
  async getSelectedText(terminalId: string): Promise<string> {
    const sel = this.getSelection();
    if (!sel) return '';
    try {
      return await invoke<string>('get_grid_text', {
        terminalId,
        startRow: sel.startRow,
        startCol: sel.startCol,
        endRow: sel.endRow,
        endCol: sel.endCol,
      });
    } catch {
      return '';
    }
  }

  /** Focus the canvas for keyboard input. */
  focus() {
    this.canvas.focus();
  }

  /** Clean up all resources. */
  dispose() {
    if (this.cursorBlinkInterval) {
      clearInterval(this.cursorBlinkInterval);
      this.cursorBlinkInterval = null;
    }
    if (this.onDocumentMouseMove) {
      document.removeEventListener('mousemove', this.onDocumentMouseMove);
      this.onDocumentMouseMove = null;
    }
    if (this.onDocumentMouseUp) {
      document.removeEventListener('mouseup', this.onDocumentMouseUp);
      this.onDocumentMouseUp = null;
    }
    if (this.webglRenderer) {
      this.webglRenderer.dispose();
      this.webglRenderer = null;
    }
  }

  // ---- Scrollback ----

  /** Scroll to the bottom (live view) by requesting offset 0 via the scroll callback. */
  scrollToBottom() {
    if (this.onScrollCallback) {
      // Use a very large negative delta to ensure we reach offset 0
      const currentOffset = this.currentSnapshot?.scrollback_offset ?? 0;
      if (currentOffset > 0) {
        this.onScrollCallback(-currentOffset);
      }
    }
  }

  /** Returns the current scrollback offset from the latest snapshot. */
  getScrollbackOffset(): number {
    return this.currentSnapshot?.scrollback_offset ?? 0;
  }

  // ---- Private: Font measurement ----

  private measureFont() {
    if (!this.ctx) return; // WebGL mode — font measured by WebGLRenderer
    const dpr = this.devicePixelRatio;
    // Round to integer pixel size for clean font hinting (avoids subpixel artifacts
    // at fractional DPR like 1.25 where 13*1.25=16.25 causes poor ClearType rendering)
    const scaledSize = Math.round(this.fontSize * dpr);
    this.ctx.font = `${scaledSize}px ${this.fontFamily}`;
    const metrics = this.ctx.measureText('M');
    this.cellWidth = Math.ceil(metrics.width);
    // Line height = fontSize * 1.2 is a reasonable approximation
    this.cellHeight = Math.ceil(scaledSize * 1.2);
    // Baseline offset: distance from the top of the cell to the text baseline
    this.baselineOffset = Math.ceil(scaledSize);
  }

  // ---- Private: Painting ----

  private paint() {
    const snap = this.currentSnapshot;
    if (!snap) return;

    perfTracer.mark('paint_start');

    if (this.useWebGL && this.webglRenderer) {
      // WebGL path: delegate grid rendering to GPU
      const sel = this.selection.active ? this.normalizeSelection(this.selection) : null;
      this.webglRenderer.paint(snap, this.theme, sel, this.cursorVisible && !snap.cursor_hidden && snap.scrollback_offset === 0);
      perfTracer.measure('webgl_paint', 'paint_start');
      // Draw scrollbar and URL hover on overlay
      this.paintOverlay(snap);
      return;
    }

    // Canvas2D fallback — two-pass rendering to prevent background rects
    // from clipping adjacent glyphs (e.g. 'm' trailing edge cut by next cell's bg)
    const ctx = this.ctx!;
    const { cellWidth, cellHeight, baselineOffset } = this;
    const dpr = this.devicePixelRatio;
    const scaledSize = Math.round(this.fontSize * dpr);

    // Clear canvas with background
    ctx.fillStyle = this.theme.background;
    ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

    const normalizedSel = this.selection.active ? this.normalizeSelection(this.selection) : null;

    // Pass 1: Draw all backgrounds
    for (let row = 0; row < snap.rows.length; row++) {
      const gridRow = snap.rows[row];
      const y = row * cellHeight;

      for (let col = 0; col < gridRow.cells.length; col++) {
        const cell = gridRow.cells[col];
        if (cell.wide_continuation) continue;

        const x = col * cellWidth;
        const w = cell.wide ? cellWidth * 2 : cellWidth;

        let bg = cell.bg === 'default' ? this.theme.background : cell.bg;
        if (cell.inverse) {
          bg = cell.fg === 'default' ? this.theme.foreground : cell.fg;
        }

        const inSelection = normalizedSel && this.cellInSelection(row, col, normalizedSel);

        if (inSelection) {
          ctx.fillStyle = this.theme.selectionBackground;
          ctx.fillRect(x, y, w, cellHeight);
        } else if (bg !== this.theme.background) {
          ctx.fillStyle = bg;
          ctx.fillRect(x, y, w, cellHeight);
        }
      }
    }

    // Pass 2: Draw all text and decorations (on top of backgrounds)
    for (let row = 0; row < snap.rows.length; row++) {
      const gridRow = snap.rows[row];
      const y = row * cellHeight;

      for (let col = 0; col < gridRow.cells.length; col++) {
        const cell = gridRow.cells[col];
        if (cell.wide_continuation) continue;

        const x = col * cellWidth;
        const w = cell.wide ? cellWidth * 2 : cellWidth;

        let fg = cell.fg === 'default' ? this.theme.foreground : cell.fg;
        if (cell.inverse) {
          fg = cell.bg === 'default' ? this.theme.background : cell.bg;
        }
        if (cell.dim) {
          fg = this.dimColor(fg);
        }

        const inSelection = normalizedSel && this.cellInSelection(row, col, normalizedSel);

        // Draw text
        if (cell.content && cell.content !== ' ') {
          const fontWeight = cell.bold ? 'bold ' : '';
          const fontStyle = cell.italic ? 'italic ' : '';
          ctx.font = `${fontStyle}${fontWeight}${scaledSize}px ${this.fontFamily}`;

          if (inSelection) {
            ctx.fillStyle = this.theme.foreground;
          } else {
            ctx.fillStyle = fg;
          }

          ctx.fillText(cell.content, x, y + baselineOffset);
        }

        // Draw underline
        if (cell.underline) {
          ctx.strokeStyle = fg;
          ctx.lineWidth = dpr;
          ctx.beginPath();
          ctx.moveTo(x, y + cellHeight - dpr);
          ctx.lineTo(x + w, y + cellHeight - dpr);
          ctx.stroke();
        }
      }

      // URL hover underline
      if (this.hoveredUrl && row === this.hoveredUrlRow) {
        const urlX = this.hoveredUrlStartCol * cellWidth;
        const urlW = (this.hoveredUrlEndCol - this.hoveredUrlStartCol) * cellWidth;
        ctx.strokeStyle = this.theme.blue;
        ctx.lineWidth = dpr;
        ctx.beginPath();
        ctx.moveTo(urlX, y + cellHeight - dpr);
        ctx.lineTo(urlX + urlW, y + cellHeight - dpr);
        ctx.stroke();
      }
    }

    // Draw cursor (only when at live view — no cursor when scrolled up)
    if (!snap.cursor_hidden && this.cursorVisible && snap.scrollback_offset === 0) {
      this.paintCursor(snap.cursor.row, snap.cursor.col);
    }

    // Draw scrollbar indicator when scrolled into history
    this.paintScrollbar(snap);
  }

  private paintCursor(row: number, col: number) {
    const ctx = this.ctx!;
    const x = col * this.cellWidth;
    const y = row * this.cellHeight;
    const dpr = this.devicePixelRatio;

    // Block cursor
    ctx.fillStyle = this.theme.cursor;
    ctx.globalAlpha = 0.7;
    ctx.fillRect(x, y, this.cellWidth, this.cellHeight);
    ctx.globalAlpha = 1.0;

    // Draw the character under the cursor with the accent color
    if (this.currentSnapshot) {
      const gridRow = this.currentSnapshot.rows[row];
      if (gridRow) {
        const cell = gridRow.cells[col];
        if (cell && cell.content && cell.content !== ' ') {
          const scaledSize = Math.round(this.fontSize * dpr);
          ctx.font = `${scaledSize}px ${this.fontFamily}`;
          ctx.fillStyle = this.theme.cursorAccent;
          ctx.fillText(cell.content, x, y + this.baselineOffset);
        }
      }
    }
  }

  // ---- Private: Color helpers ----

  private dimColor(hex: string): string {
    // Dim by reducing luminance by ~33%
    if (!hex.startsWith('#') || hex.length < 7) return hex;
    const r = parseInt(hex.slice(1, 3), 16);
    const g = parseInt(hex.slice(3, 5), 16);
    const b = parseInt(hex.slice(5, 7), 16);
    const dim = (v: number) => Math.round(v * 0.67);
    return `#${dim(r).toString(16).padStart(2, '0')}${dim(g).toString(16).padStart(2, '0')}${dim(b).toString(16).padStart(2, '0')}`;
  }

  // ---- Private: Selection ----

  private normalizeSelection(sel: Selection): Selection {
    if (sel.startRow < sel.endRow || (sel.startRow === sel.endRow && sel.startCol <= sel.endCol)) {
      return sel;
    }
    return {
      startRow: sel.endRow,
      startCol: sel.endCol,
      endRow: sel.startRow,
      endCol: sel.startCol,
      active: sel.active,
    };
  }

  private cellInSelection(row: number, col: number, sel: Selection): boolean {
    if (row < sel.startRow || row > sel.endRow) return false;
    if (row === sel.startRow && row === sel.endRow) {
      return col >= sel.startCol && col < sel.endCol;
    }
    if (row === sel.startRow) return col >= sel.startCol;
    if (row === sel.endRow) return col < sel.endCol;
    return true;
  }

  private pixelToGrid(clientX: number, clientY: number): { row: number; col: number } {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = this.devicePixelRatio;
    const cssX = clientX - rect.left;
    const cssY = clientY - rect.top;
    const canvasX = cssX * dpr;
    const canvasY = cssY * dpr;
    const col = Math.floor(canvasX / this.cellWidth);
    const row = Math.floor(canvasY / this.cellHeight);
    return { row: Math.max(0, row), col: Math.max(0, col) };
  }

  // ---- Private: Mouse handlers ----

  private setupMouseHandlers() {
    this.canvas.addEventListener('mousedown', (e) => {
      if (e.button !== 0) return; // left button only

      // Check if click is in the scrollbar hit area (right N CSS px)
      if (this.isInScrollbarHitArea(e.clientX)) {
        e.preventDefault();
        this.startScrollbarDrag(e);
        return;
      }

      const { row, col } = this.pixelToGrid(e.clientX, e.clientY);
      this.selection = {
        startRow: row,
        startCol: col,
        endRow: row,
        endCol: col,
        active: false,
      };
      this.isSelecting = true;
    });

    this.canvas.addEventListener('mousemove', (e) => {
      // Scrollbar drag is handled by document-level listeners
      if (this.isDraggingScrollbar) return;

      const { row, col } = this.pixelToGrid(e.clientX, e.clientY);

      if (this.isSelecting) {
        this.selection.endRow = row;
        this.selection.endCol = col;
        // Only mark as active once we've moved at least one cell
        if (row !== this.selection.startRow || col !== this.selection.startCol) {
          this.selection.active = true;
        }
        this.repaint();
        return;
      }

      // URL hover detection
      this.detectUrlHover(row, col, e.ctrlKey);

      // Update cursor style for URLs or scrollbar
      if (e.ctrlKey && this.hoveredUrl) {
        this.canvas.style.cursor = 'pointer';
      } else if (this.isInScrollbarHitArea(e.clientX) && this.currentSnapshot && this.currentSnapshot.total_scrollback > 0) {
        this.canvas.style.cursor = 'default';
      } else {
        this.canvas.style.cursor = 'default';
      }
    });

    this.canvas.addEventListener('mouseup', () => {
      this.isSelecting = false;
    });

    // Ctrl+click to open URLs
    this.canvas.addEventListener('click', (e) => {
      if (e.ctrlKey && this.hoveredUrl) {
        const url = this.hoveredUrl;
        import('@tauri-apps/plugin-opener').then(({ openUrl }) => {
          openUrl(url).catch((err: unknown) => {
            console.error('Failed to open URL:', err);
          });
        });
      }
    });
  }

  /** Check if a clientX coordinate is within the scrollbar hit area. */
  private isInScrollbarHitArea(clientX: number): boolean {
    const snap = this.currentSnapshot;
    if (!snap || snap.total_scrollback <= 0) return false;
    const rect = this.canvas.getBoundingClientRect();
    const cssX = clientX - rect.left;
    return cssX >= rect.width - TerminalRenderer.SCROLLBAR_HIT_WIDTH_CSS;
  }

  /** Start a scrollbar drag from a mousedown event. */
  private startScrollbarDrag(e: MouseEvent) {
    this.isDraggingScrollbar = true;
    // Immediately jump to the clicked position
    this.handleScrollbarDragAt(e.clientY);

    this.onDocumentMouseMove = (moveEvent: MouseEvent) => {
      moveEvent.preventDefault();
      this.handleScrollbarDragAt(moveEvent.clientY);
    };
    this.onDocumentMouseUp = () => {
      this.isDraggingScrollbar = false;
      if (this.onDocumentMouseMove) {
        document.removeEventListener('mousemove', this.onDocumentMouseMove);
        this.onDocumentMouseMove = null;
      }
      if (this.onDocumentMouseUp) {
        document.removeEventListener('mouseup', this.onDocumentMouseUp);
        this.onDocumentMouseUp = null;
      }
    };

    document.addEventListener('mousemove', this.onDocumentMouseMove);
    document.addEventListener('mouseup', this.onDocumentMouseUp);
  }

  /** Convert a clientY to a scroll offset and fire the callback. */
  private handleScrollbarDragAt(clientY: number) {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = this.devicePixelRatio;
    const canvasY = (clientY - rect.top) * dpr;
    const offset = this.yToScrollOffset(canvasY);
    if (this.onScrollToCallback) {
      this.onScrollToCallback(offset);
    }
  }

  // ---- Private: URL detection ----

  private detectUrlHover(row: number, col: number, ctrlKey: boolean) {
    if (!this.currentSnapshot || !ctrlKey) {
      if (this.hoveredUrl) {
        this.hoveredUrl = null;
        this.repaint();
      }
      return;
    }

    const gridRow = this.currentSnapshot.rows[row];
    if (!gridRow) {
      this.hoveredUrl = null;
      return;
    }

    // Build the row text
    let rowText = '';
    for (const cell of gridRow.cells) {
      rowText += cell.content || ' ';
    }

    // Find URLs in the row
    let found = false;
    let match: RegExpExecArray | null;
    URL_REGEX.lastIndex = 0;
    while ((match = URL_REGEX.exec(rowText)) !== null) {
      const start = match.index;
      const end = start + match[0].length;
      if (col >= start && col < end) {
        this.hoveredUrl = match[0];
        this.hoveredUrlRow = row;
        this.hoveredUrlStartCol = start;
        this.hoveredUrlEndCol = end;
        found = true;
        this.repaint();
        break;
      }
    }

    if (!found && this.hoveredUrl) {
      this.hoveredUrl = null;
      this.repaint();
    }
  }

  // ---- Private: Wheel handler ----

  private setupWheelHandler() {
    this.canvas.addEventListener('wheel', (e) => {
      e.preventDefault();
      if (!this.onScrollCallback) return;

      // Convert pixel delta to lines (3 lines per standard wheel tick of 100px)
      const LINES_PER_TICK = 3;
      const lines = Math.round((e.deltaY / 100) * LINES_PER_TICK) || (e.deltaY > 0 ? 1 : -1);

      // deltaY > 0 = scroll down in page terms = scroll toward live (negative delta)
      // deltaY < 0 = scroll up in page terms = scroll into history (positive delta)
      this.onScrollCallback(-lines);
    }, { passive: false });
  }

  // ---- Private: Scrollbar geometry + painting ----

  /** Scrollbar hit area width in CSS pixels. */
  private static readonly SCROLLBAR_HIT_WIDTH_CSS = 20;

  /**
   * Compute scrollbar geometry for hit testing and painting.
   * Returns null if there is no scrollable content.
   */
  private getScrollbarGeometry(): {
    trackX: number;
    trackWidth: number;
    trackPadding: number;
    thumbY: number;
    thumbHeight: number;
    canvasHeight: number;
    canvasWidth: number;
    totalRange: number;
  } | null {
    const snap = this.currentSnapshot;
    if (!snap || snap.total_scrollback <= 0) return null;

    const dpr = this.devicePixelRatio;
    const canvasHeight = this.canvas.height;
    const canvasWidth = this.canvas.width;
    const trackWidth = 6 * dpr;
    const trackX = canvasWidth - trackWidth;
    const trackPadding = 2 * dpr;

    const totalRange = snap.total_scrollback;
    const visibleRows = snap.dimensions.rows;
    const totalContent = totalRange + visibleRows;
    const thumbHeight = Math.max(20 * dpr, (visibleRows / totalContent) * (canvasHeight - trackPadding * 2));

    const scrollFraction = snap.scrollback_offset / totalRange;
    const trackHeight = canvasHeight - trackPadding * 2 - thumbHeight;
    const thumbY = trackPadding + trackHeight * (1 - scrollFraction);

    return { trackX, trackWidth, trackPadding, thumbY, thumbHeight, canvasHeight, canvasWidth, totalRange };
  }

  /**
   * Convert a canvas-relative Y coordinate to an absolute scroll offset
   * using the same geometry as the scrollbar.
   */
  private yToScrollOffset(canvasY: number): number {
    const geo = this.getScrollbarGeometry();
    if (!geo) return 0;

    const { trackPadding, thumbHeight, canvasHeight, totalRange } = geo;
    const trackHeight = canvasHeight - trackPadding * 2 - thumbHeight;
    if (trackHeight <= 0) return 0;

    // Clamp Y to the center of the thumb range
    const thumbCenter = canvasY - trackPadding - thumbHeight / 2;
    const fraction = 1 - Math.max(0, Math.min(1, thumbCenter / trackHeight));
    return Math.round(fraction * totalRange);
  }

  private paintScrollbar(snap: RichGridData) {
    if (snap.scrollback_offset <= 0 || snap.total_scrollback <= 0) return;

    const ctx = this.ctx!;
    const canvasHeight = this.canvas.height;
    const canvasWidth = this.canvas.width;
    const dpr = this.devicePixelRatio;

    const trackWidth = 6 * dpr;
    const trackX = canvasWidth - trackWidth;
    const trackPadding = 2 * dpr;

    // Track background
    ctx.fillStyle = 'rgba(255, 255, 255, 0.1)';
    ctx.fillRect(trackX, trackPadding, trackWidth, canvasHeight - trackPadding * 2);

    // Thumb: position reflects where in scrollback we are
    const totalRange = snap.total_scrollback;
    const visibleRows = snap.dimensions.rows;
    const totalContent = totalRange + visibleRows;
    const thumbHeight = Math.max(20 * dpr, (visibleRows / totalContent) * (canvasHeight - trackPadding * 2));

    // offset=0 is bottom (live), offset=totalScrollback is top
    const scrollFraction = snap.scrollback_offset / totalRange;
    const trackHeight = canvasHeight - trackPadding * 2 - thumbHeight;
    const thumbY = trackPadding + trackHeight * (1 - scrollFraction);

    ctx.fillStyle = 'rgba(255, 255, 255, 0.4)';
    ctx.fillRect(trackX + 1 * dpr, thumbY, trackWidth - 2 * dpr, thumbHeight);
  }

  // ---- Private: WebGL overlay painting ----

  private paintOverlay(snap: RichGridData) {
    if (!this.overlayCanvas || !this.overlayCtx) return;
    const ctx = this.overlayCtx;
    const dpr = this.devicePixelRatio;
    ctx.clearRect(0, 0, this.overlayCanvas.width, this.overlayCanvas.height);

    // Scrollbar
    if (snap.scrollback_offset > 0 && snap.total_scrollback > 0) {
      const canvasHeight = this.overlayCanvas.height;
      const canvasWidth = this.overlayCanvas.width;
      const trackWidth = 6 * dpr;
      const trackX = canvasWidth - trackWidth;
      const trackPadding = 2 * dpr;
      ctx.fillStyle = 'rgba(255, 255, 255, 0.1)';
      ctx.fillRect(trackX, trackPadding, trackWidth, canvasHeight - trackPadding * 2);
      const totalRange = snap.total_scrollback;
      const visibleRows = snap.dimensions.rows;
      const totalContent = totalRange + visibleRows;
      const thumbHeight = Math.max(20 * dpr, (visibleRows / totalContent) * (canvasHeight - trackPadding * 2));
      const scrollFraction = snap.scrollback_offset / totalRange;
      const trackHeight = canvasHeight - trackPadding * 2 - thumbHeight;
      const thumbY = trackPadding + trackHeight * (1 - scrollFraction);
      ctx.fillStyle = 'rgba(255, 255, 255, 0.4)';
      ctx.fillRect(trackX + 1 * dpr, thumbY, trackWidth - 2 * dpr, thumbHeight);
    }

    // URL hover underline
    if (this.hoveredUrl && this.hoveredUrlRow >= 0) {
      const urlX = this.hoveredUrlStartCol * this.cellWidth;
      const urlW = (this.hoveredUrlEndCol - this.hoveredUrlStartCol) * this.cellWidth;
      const y = this.hoveredUrlRow * this.cellHeight;
      ctx.strokeStyle = this.theme.blue;
      ctx.lineWidth = dpr;
      ctx.beginPath();
      ctx.moveTo(urlX, y + this.cellHeight - dpr);
      ctx.lineTo(urlX + urlW, y + this.cellHeight - dpr);
      ctx.stroke();
    }
  }

  /** Get the overlay canvas element (WebGL mode only). */
  getOverlayElement(): HTMLCanvasElement | null {
    return this.overlayCanvas;
  }

  // ---- Private: Cursor blink ----

  private startCursorBlink() {
    this.cursorBlinkInterval = setInterval(() => {
      this.cursorVisible = !this.cursorVisible;
      if (this.currentSnapshot) {
        this.repaint();
      }
    }, 600);
  }
}
