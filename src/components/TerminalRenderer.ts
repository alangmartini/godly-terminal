/**
 * Canvas2D terminal renderer.
 *
 * Replaces xterm.js by painting grid snapshots from godly-vt (via Tauri IPC)
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

// ---- Theme (matches xterm.js theme from TerminalPane.ts) ----

export interface TerminalTheme {
  background: string;
  foreground: string;
  cursor: string;
  cursorAccent: string;
  selectionBackground: string;
  black: string;
  red: string;
  green: string;
  yellow: string;
  blue: string;
  magenta: string;
  cyan: string;
  white: string;
  brightBlack: string;
  brightRed: string;
  brightGreen: string;
  brightYellow: string;
  brightBlue: string;
  brightMagenta: string;
  brightCyan: string;
  brightWhite: string;
}

export const DEFAULT_THEME: TerminalTheme = {
  background: '#1e1e1e',
  foreground: '#cccccc',
  cursor: '#aeafad',
  cursorAccent: '#1e1e1e',
  selectionBackground: '#264f78',
  black: '#000000',
  red: '#cd3131',
  green: '#0dbc79',
  yellow: '#e5e510',
  blue: '#2472c8',
  magenta: '#bc3fbc',
  cyan: '#11a8cd',
  white: '#e5e5e5',
  brightBlack: '#666666',
  brightRed: '#f14c4c',
  brightGreen: '#23d18b',
  brightYellow: '#f5f543',
  brightBlue: '#3b8eea',
  brightMagenta: '#d670d6',
  brightCyan: '#29b8db',
  brightWhite: '#e5e5e5',
};

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

// ---- Renderer ----

export class TerminalRenderer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private theme: TerminalTheme;

  // Font metrics
  private fontFamily = 'Cascadia Code, Consolas, monospace';
  private fontSize = 14;
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

  // Callbacks
  private onTitleChange?: (title: string) => void;
  private onScrollCallback?: (deltaLines: number) => void;

  constructor(theme?: Partial<TerminalTheme>) {
    this.theme = { ...DEFAULT_THEME, ...theme };
    this.canvas = document.createElement('canvas');
    this.canvas.className = 'terminal-canvas';
    this.canvas.style.display = 'block';
    this.canvas.style.width = '100%';
    this.canvas.style.height = '100%';
    this.canvas.tabIndex = 0;
    // Prevent default context menu on right-click (we handle copy ourselves)
    this.canvas.addEventListener('contextmenu', (e) => e.preventDefault());
    this.ctx = this.canvas.getContext('2d', { alpha: false })!;
    this.measureFont();
    this.setupMouseHandlers();
    this.setupWheelHandler();
    this.startCursorBlink();
  }

  /** Get the canvas element for mounting into the DOM. */
  getElement(): HTMLCanvasElement {
    return this.canvas;
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
      requestAnimationFrame(() => {
        this.renderScheduled = false;
        if (this.pendingSnapshot) {
          this.currentSnapshot = this.pendingSnapshot;
          this.pendingSnapshot = null;
          this.paint();
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
    const scaledSize = this.fontSize * dpr;
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

    const ctx = this.ctx;
    const { cellWidth, cellHeight, baselineOffset } = this;
    const dpr = this.devicePixelRatio;

    // Clear canvas with background
    ctx.fillStyle = this.theme.background;
    ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);

    const normalizedSel = this.selection.active ? this.normalizeSelection(this.selection) : null;

    // Paint each row
    for (let row = 0; row < snap.rows.length; row++) {
      const gridRow = snap.rows[row];
      const y = row * cellHeight;

      for (let col = 0; col < gridRow.cells.length; col++) {
        const cell = gridRow.cells[col];

        // Skip wide continuation cells (the wide character occupies 2 cells)
        if (cell.wide_continuation) continue;

        const x = col * cellWidth;
        const w = cell.wide ? cellWidth * 2 : cellWidth;

        // Resolve colors
        let fg = cell.fg === 'default' ? this.theme.foreground : cell.fg;
        let bg = cell.bg === 'default' ? this.theme.background : cell.bg;

        // Handle inverse video
        if (cell.inverse) {
          const tmp = fg;
          fg = bg;
          bg = tmp;
        }

        // Handle dim
        if (cell.dim) {
          fg = this.dimColor(fg);
        }

        // Check if this cell is in the selection
        const inSelection = normalizedSel && this.cellInSelection(row, col, normalizedSel);

        // Draw background
        if (inSelection) {
          ctx.fillStyle = this.theme.selectionBackground;
          ctx.fillRect(x, y, w, cellHeight);
        } else if (bg !== this.theme.background) {
          ctx.fillStyle = bg;
          ctx.fillRect(x, y, w, cellHeight);
        }

        // Draw text
        if (cell.content && cell.content !== ' ') {
          // Build font string
          const fontWeight = cell.bold ? 'bold ' : '';
          const fontStyle = cell.italic ? 'italic ' : '';
          const scaledSize = this.fontSize * dpr;
          ctx.font = `${fontStyle}${fontWeight}${scaledSize}px ${this.fontFamily}`;

          if (inSelection) {
            // Use foreground on selection background for readability
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
    const ctx = this.ctx;
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
          const scaledSize = this.fontSize * dpr;
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

      // Update cursor style for URLs
      if (e.ctrlKey && this.hoveredUrl) {
        this.canvas.style.cursor = 'pointer';
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

  // ---- Private: Scrollbar indicator ----

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
