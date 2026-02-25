/**
 * Terminal interaction overlay renderer.
 *
 * Handles all user interaction visuals (selection highlights, scrollbar,
 * URL hover underline) on a transparent <canvas> overlay. The actual grid
 * painting is done by the GPU renderer (Rust-side wgpu) via GpuTerminalDisplay.
 *
 * Also handles all mouse, wheel, and touch events for selection, scrollbar
 * drag, URL hover/click, and zoom.
 */

import { invoke } from '@tauri-apps/api/core';
import { perfTracer } from '../utils/PerfTracer';
import { themeStore } from '../state/theme-store';
import { terminalSettingsStore } from '../state/terminal-settings-store';
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

// ---- Renderer backend info ----

/** Returns the rendering backend name. Always 'GPU' now. */
export function getRendererBackend(): string {
  return 'GPU';
}

// ---- Renderer ----

export class TerminalRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private theme: TerminalTheme;

  // Font metrics
  private fontFamily = 'Cascadia Code, Consolas, monospace';
  private fontSize = terminalSettingsStore.getFontSize();
  private cellWidth = 0;
  private cellHeight = 0;
  private devicePixelRatio = 1;

  // State
  private currentSnapshot: RichGridData | null = null;
  private pendingSnapshot: RichGridData | null = null;
  private renderScheduled = false;

  // Cursor blink (triggers GPU frame requests via repaint callback)
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

  // Callback fired when a mouse drag selection ends (mouseup after selecting)
  private onSelectionEndCallback?: () => void;

  // URL hover
  private hoveredUrl: string | null = null;
  private hoveredUrlRow = -1;
  private hoveredUrlStartCol = -1;
  private hoveredUrlEndCol = -1;

  // Scrollbar drag state
  private isDraggingScrollbar = false;
  private onDocumentMouseMove: ((e: MouseEvent) => void) | null = null;
  private onDocumentMouseUp: ((e: MouseEvent) => void) | null = null;

  // Selection auto-scroll
  private autoScrollTimer: ReturnType<typeof setInterval> | null = null;
  private autoScrollDelta = 0;
  private onDocumentSelectionMouseMove: ((e: MouseEvent) => void) | null = null;
  private onDocumentSelectionMouseUp: ((e: MouseEvent) => void) | null = null;

  // Touch scroll state
  private touchStartY: number | null = null;
  private touchAccumulated = 0;

  // Callbacks
  private onTitleChange?: (title: string) => void;
  private onScrollCallback?: (deltaLines: number) => void;
  private onScrollToCallback?: (absoluteOffset: number) => void;
  private onZoomCallback?: (delta: number) => void;

  constructor(theme?: Partial<TerminalTheme>) {
    this.theme = theme ? { ...themeStore.getTerminalTheme(), ...theme } : themeStore.getTerminalTheme();

    // The canvas serves as a transparent overlay for selection, scrollbar, and URL hover.
    // The GPU renderer paints the grid on a separate canvas underneath.
    this.canvas = document.createElement('canvas');
    this.canvas.className = 'terminal-overlay-canvas';
    this.canvas.style.display = 'block';
    this.canvas.tabIndex = 0;
    // Prevent default context menu on right-click (we handle copy ourselves)
    this.canvas.addEventListener('contextmenu', (e) => e.preventDefault());

    // Alpha-enabled context so the GPU-rendered grid shows through
    this.ctx = this.canvas.getContext('2d', { alpha: true })!;
    this.measureFont();
    console.log('[TerminalRenderer] Initialized as interaction overlay (GPU renders grid)');

    this.setupMouseHandlers();
    this.setupWheelHandler();
    this.setupTouchHandler();
    this.startCursorBlink();
  }

  /** Get the canvas element for mounting into the DOM. */
  getElement(): HTMLCanvasElement {
    return this.canvas;
  }

  /** Returns the active rendering backend name. */
  getBackend(): string {
    return 'GPU';
  }

  /** Update the terminal theme and trigger a repaint. */
  setTheme(theme: TerminalTheme): void {
    this.theme = theme;
    this.repaint();
  }

  /** Update font size and re-measure. Triggers repaint. */
  setFontSize(size: number): void {
    if (size === this.fontSize) return;
    this.fontSize = size;
    this.measureFont();
    this.repaint();
  }

  /** Set zoom callback. delta > 0 = zoom in, < 0 = zoom out. */
  setOnZoom(cb: (delta: number) => void) {
    this.onZoomCallback = cb;
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

  /** Set callback for when a drag selection ends (mouseup after drag). */
  setOnSelectionEnd(cb: () => void) {
    this.onSelectionEndCallback = cb;
  }

  /** Returns true while the user is actively dragging to select text. */
  isActivelySelecting(): boolean {
    return this.isSelecting;
  }

  /**
   * Schedule a render with the given snapshot.
   * Updates snapshot state for selection/scrollbar reference and repaints the overlay.
   * The GPU renderer handles actual grid painting separately.
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
          this.paintOverlay();
          perfTracer.measure('paint_duration', 'paint_start');
          perfTracer.measure('keydown_to_paint', 'keydown');
          perfTracer.tick();
        }
      });
    }
  }

  /** Force an immediate re-render of the overlay. */
  repaint() {
    if (this.currentSnapshot) {
      this.paintOverlay();
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
    this.measureFont();
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

  /**
   * Adjust selection coordinates when the viewport scrolls.
   * Keeps the selection anchored to the same absolute content position.
   * deltaLines > 0 = scrolled up (into history), rows shift down in viewport.
   * deltaLines < 0 = scrolled down (toward live), rows shift up in viewport.
   */
  adjustSelectionForScroll(deltaLines: number) {
    if (!this.selection.active) return;
    this.selection.startRow += deltaLines;
    // Bug #340: During active drag, only adjust the anchor (startRow).
    // endRow tracks the mouse position in viewport coordinates and should
    // not be shifted — this lets the selection grow as the user scrolls.
    if (!this.isSelecting) {
      this.selection.endRow += deltaLines;
    }
    // Bug #290: Only clear off-screen selection when not actively auto-scrolling.
    // Bug #340: Also don't clear during active drag — the anchor may be
    // off-screen but the selection is still valid (extends beyond viewport).
    if (!this.autoScrollTimer && !this.isSelecting) {
      const gridRows = this.currentSnapshot?.dimensions.rows ?? 24;
      const normalized = this.normalizeSelection(this.selection);
      if (normalized.endRow < 0 || normalized.startRow >= gridRows) {
        this.selection.active = false;
      }
    }
  }

  /** Clear the current selection. */
  clearSelection() {
    this.selection.active = false;
    this.repaint();
  }

  /**
   * Get selected text by calling the backend.
   * Passes the current scrollback offset so the backend can convert
   * viewport-relative selection coordinates to absolute buffer positions,
   * supporting multi-screen selections that span more rows than the viewport.
   */
  async getSelectedText(terminalId: string): Promise<string> {
    const sel = this.getSelection();
    if (!sel) return '';
    const scrollbackOffset = this.currentSnapshot?.scrollback_offset ?? 0;
    try {
      return await invoke<string>('get_grid_text', {
        terminalId,
        startRow: sel.startRow,
        startCol: sel.startCol,
        endRow: sel.endRow,
        endCol: sel.endCol,
        scrollbackOffset,
      });
    } catch {
      return '';
    }
  }

  /** Focus the canvas for keyboard input. */
  focus() {
    this.canvas.focus();
  }

  /**
   * Release canvas resources without destroying the renderer.
   * Called when the terminal is paused (hidden tab). The canvas stays
   * in the DOM but its backing store is freed by setting dimensions to 1x1.
   * Call restoreCanvasResources() to re-allocate when the terminal becomes visible.
   */
  releaseCanvasResources() {
    // Shrink canvas to 1x1 to release GPU backing store.
    this.canvas.width = 1;
    this.canvas.height = 1;
    // Drop cached snapshot data
    this.currentSnapshot = null;
    this.pendingSnapshot = null;
    // Stop cursor blink timer (no need to repaint hidden canvas)
    if (this.cursorBlinkInterval) {
      clearInterval(this.cursorBlinkInterval);
      this.cursorBlinkInterval = null;
    }
  }

  /**
   * Re-allocate canvas resources after releaseCanvasResources().
   * Called when the terminal becomes visible again. updateSize() will
   * set the correct dimensions; startCursorBlink() restarts the timer.
   */
  restoreCanvasResources() {
    // Restart cursor blink if it was stopped
    if (!this.cursorBlinkInterval) {
      this.startCursorBlink();
    }
  }

  /**
   * No-op retained for API compatibility with TerminalPane.
   * WebGL promotion is no longer needed since the GPU renderer
   * handles all grid painting.
   */
  promoteToWebGL(): boolean {
    return false;
  }

  /** Returns null since there is no separate overlay canvas. The main canvas IS the overlay. */
  getOverlayElement(): HTMLCanvasElement | null {
    return null;
  }

  /** Clean up all resources. */
  dispose() {
    this.stopSelectionAutoScroll();
    this.removeDocumentSelectionListeners();
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
    // Release canvas backing store
    this.canvas.width = 1;
    this.canvas.height = 1;
    this.currentSnapshot = null;
    this.pendingSnapshot = null;
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
    const dpr = this.devicePixelRatio;
    // Round to integer pixel size for clean font hinting (avoids subpixel artifacts
    // at fractional DPR like 1.25 where 13*1.25=16.25 causes poor ClearType rendering)
    const scaledSize = Math.round(this.fontSize * dpr);
    this.ctx.font = `${scaledSize}px ${this.fontFamily}`;
    const metrics = this.ctx.measureText('M');
    this.cellWidth = Math.ceil(metrics.width);
    // Line height = fontSize * 1.2 is a reasonable approximation
    this.cellHeight = Math.ceil(scaledSize * 1.2);
  }

  // ---- Private: Overlay painting ----

  /**
   * Paint the interaction overlay: selection highlights, scrollbar, URL hover.
   * This is drawn on a transparent canvas that sits on top of the GPU-rendered grid.
   */
  private paintOverlay() {
    const snap = this.currentSnapshot;
    if (!snap) return;

    perfTracer.mark('paint_start');

    const ctx = this.ctx;
    const dpr = this.devicePixelRatio;
    ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);

    // Selection highlight
    const normalizedSel = this.selection.active ? this.normalizeSelection(this.selection) : null;
    if (normalizedSel) {
      ctx.fillStyle = this.theme.selectionBackground;
      for (let row = normalizedSel.startRow; row <= normalizedSel.endRow; row++) {
        if (row < 0 || row >= snap.dimensions.rows) continue;
        const y = row * this.cellHeight;

        let startCol: number;
        let endCol: number;
        if (row === normalizedSel.startRow && row === normalizedSel.endRow) {
          startCol = normalizedSel.startCol;
          endCol = normalizedSel.endCol;
        } else if (row === normalizedSel.startRow) {
          startCol = normalizedSel.startCol;
          endCol = snap.dimensions.cols;
        } else if (row === normalizedSel.endRow) {
          startCol = 0;
          endCol = normalizedSel.endCol;
        } else {
          startCol = 0;
          endCol = snap.dimensions.cols;
        }

        const x = startCol * this.cellWidth;
        const w = (endCol - startCol) * this.cellWidth;
        ctx.fillRect(x, y, w, this.cellHeight);
      }
    }

    // Scrollbar
    if (snap.scrollback_offset > 0 && snap.total_scrollback > 0) {
      this.paintScrollbar(snap);
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

  /** Unclamped variant of pixelToGrid — returns raw row/col for edge detection during selection. */
  private pixelToGridRaw(clientX: number, clientY: number): { row: number; col: number } {
    const rect = this.canvas.getBoundingClientRect();
    const dpr = this.devicePixelRatio;
    const cssX = clientX - rect.left;
    const cssY = clientY - rect.top;
    const canvasX = cssX * dpr;
    const canvasY = cssY * dpr;
    const col = Math.floor(canvasX / this.cellWidth);
    const row = Math.floor(canvasY / this.cellHeight);
    return { row, col };
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

      // Register document-level listeners so we receive events even when cursor leaves canvas
      this.onDocumentSelectionMouseMove = (moveEvent: MouseEvent) => {
        moveEvent.preventDefault();
        const gridRows = this.currentSnapshot?.dimensions.rows ?? 24;
        const raw = this.pixelToGridRaw(moveEvent.clientX, moveEvent.clientY);
        const clamped = this.pixelToGrid(moveEvent.clientX, moveEvent.clientY);

        this.selection.endRow = Math.min(clamped.row, gridRows - 1);
        this.selection.endCol = clamped.col;
        if (this.selection.endRow !== this.selection.startRow || this.selection.endCol !== this.selection.startCol) {
          this.selection.active = true;
        }

        // Edge detection for auto-scroll
        if (raw.row < 0) {
          // Mouse above viewport -> scroll up into history
          const linesPerTick = Math.min(10, Math.ceil(Math.abs(raw.row)));
          this.startSelectionAutoScroll(linesPerTick);
        } else if (raw.row >= gridRows) {
          // Mouse below viewport -> scroll down toward live
          const linesPerTick = -Math.min(10, raw.row - gridRows + 1);
          this.startSelectionAutoScroll(linesPerTick);
        } else {
          this.stopSelectionAutoScroll();
        }

        this.repaint();
      };

      this.onDocumentSelectionMouseUp = () => {
        this.stopSelectionAutoScroll();
        const wasSelecting = this.isSelecting;
        this.isSelecting = false;
        if (wasSelecting && this.selection.active && this.onSelectionEndCallback) {
          this.onSelectionEndCallback();
        }
        this.removeDocumentSelectionListeners();
      };

      document.addEventListener('mousemove', this.onDocumentSelectionMouseMove);
      document.addEventListener('mouseup', this.onDocumentSelectionMouseUp);
    });

    this.canvas.addEventListener('mousemove', (e) => {
      // Scrollbar drag and selection drag are handled by document-level listeners
      if (this.isDraggingScrollbar || this.onDocumentSelectionMouseMove) return;

      const { row, col } = this.pixelToGrid(e.clientX, e.clientY);

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
      // Selection mouseup is handled by document-level listener when active
      if (this.onDocumentSelectionMouseMove) return;

      const wasSelecting = this.isSelecting;
      this.isSelecting = false;
      if (wasSelecting && this.selection.active && this.onSelectionEndCallback) {
        this.onSelectionEndCallback();
      }
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

  // ---- Private: Selection auto-scroll ----

  private startSelectionAutoScroll(linesPerTick: number) {
    this.autoScrollDelta = linesPerTick;
    if (this.autoScrollTimer) return; // already running, just update delta
    this.autoScrollTimer = setInterval(() => {
      if (this.onScrollCallback) {
        // Bug #290: Only call onScrollCallback — it triggers handleScroll()
        // which calls adjustSelectionForScroll() to shift both startRow and
        // endRow. Previously, startRow was also adjusted HERE, causing a
        // double-adjustment that made the anchor drift at 2x the scroll rate
        // and get clamped to viewport bounds, breaking multi-screen selections.
        this.onScrollCallback(this.autoScrollDelta);
        // Bug #290: Pin endRow to the viewport edge so the selection extends
        // into scrollback as new rows are revealed by auto-scroll.
        // Without this, endRow drifts away from the viewport edge (shifted
        // by adjustSelectionForScroll) and the selection doesn't grow.
        if (this.selection.active) {
          const gridRows = this.currentSnapshot?.dimensions.rows ?? 24;
          if (this.autoScrollDelta > 0) {
            this.selection.endRow = 0; // Scrolling up: pin to top
          } else {
            this.selection.endRow = gridRows - 1; // Scrolling down: pin to bottom
          }
        }
      }
    }, 50);
  }

  private stopSelectionAutoScroll() {
    this.autoScrollDelta = 0;
    if (this.autoScrollTimer) {
      clearInterval(this.autoScrollTimer);
      this.autoScrollTimer = null;
    }
  }

  private removeDocumentSelectionListeners() {
    if (this.onDocumentSelectionMouseMove) {
      document.removeEventListener('mousemove', this.onDocumentSelectionMouseMove);
      this.onDocumentSelectionMouseMove = null;
    }
    if (this.onDocumentSelectionMouseUp) {
      document.removeEventListener('mouseup', this.onDocumentSelectionMouseUp);
      this.onDocumentSelectionMouseUp = null;
    }
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

      // Ctrl+wheel: zoom in/out instead of scrolling
      if (e.ctrlKey && this.onZoomCallback) {
        const delta = e.deltaY < 0 ? 1 : -1;
        this.onZoomCallback(delta);
        return;
      }

      if (!this.onScrollCallback) return;

      // Convert pixel delta to lines (3 lines per standard wheel tick of 100px)
      const LINES_PER_TICK = 3;
      const lines = Math.round((e.deltaY / 100) * LINES_PER_TICK) || (e.deltaY > 0 ? 1 : -1);

      // deltaY > 0 = scroll down in page terms = scroll toward live (negative delta)
      // deltaY < 0 = scroll up in page terms = scroll into history (positive delta)
      this.onScrollCallback(-lines);
    }, { passive: false });
  }

  // ---- Private: Touch handler ----

  private setupTouchHandler() {
    this.canvas.addEventListener('touchstart', (e) => {
      if (e.touches.length === 1) {
        this.touchStartY = e.touches[0].clientY;
        this.touchAccumulated = 0;
      }
    }, { passive: true });

    this.canvas.addEventListener('touchmove', (e) => {
      if (this.touchStartY === null || e.touches.length !== 1) return;
      e.preventDefault();

      if (!this.onScrollCallback) return;

      const currentY = e.touches[0].clientY;
      const deltaPixels = this.touchStartY - currentY;
      this.touchStartY = currentY;

      // Convert pixel delta to fractional lines, accumulate to avoid losing sub-line drags
      const cellHeightCss = this.cellHeight / this.devicePixelRatio;
      this.touchAccumulated += deltaPixels / cellHeightCss;

      const lines = Math.trunc(this.touchAccumulated);
      if (lines !== 0) {
        this.touchAccumulated -= lines;
        // Swipe up (deltaPixels > 0) = scroll into history (positive delta)
        // Swipe down (deltaPixels < 0) = scroll toward live (negative delta)
        this.onScrollCallback(lines);
      }
    }, { passive: false });

    this.canvas.addEventListener('touchend', () => {
      this.touchStartY = null;
      this.touchAccumulated = 0;
    }, { passive: true });
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

    const ctx = this.ctx;
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
