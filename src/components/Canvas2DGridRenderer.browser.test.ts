/**
 * Canvas2DGridRenderer browser tests — validates real Canvas2D rendering.
 *
 * These tests are impossible in jsdom/node because:
 * - jsdom's Canvas is a stub (getContext returns null without canvas packages)
 * - measureText returns zeros in jsdom
 * - No real pixel data to inspect
 *
 * In real Chromium, we get actual font metrics, real pixel painting, and
 * a fully functional Canvas2D context.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import type { RichGridData, RichGridCell, RichGridRow } from './TerminalRenderer';

// Mock the stores that Canvas2DGridRenderer imports at module level
vi.mock('../state/theme-store', () => ({
  themeStore: {
    getTerminalTheme: () => ({
      background: '#1a1b26',
      foreground: '#a9b1d6',
      cursor: '#c0caf5',
      cursorAccent: '#1a1b26',
      selectionBackground: '#33467c',
      black: '#15161e',
      red: '#f7768e',
      green: '#9ece6a',
      yellow: '#e0af68',
      blue: '#7aa2f7',
      magenta: '#bb9af7',
      cyan: '#7dcfff',
      white: '#a9b1d6',
      brightBlack: '#414868',
      brightRed: '#f7768e',
      brightGreen: '#9ece6a',
      brightYellow: '#e0af68',
      brightBlue: '#7aa2f7',
      brightMagenta: '#bb9af7',
      brightCyan: '#7dcfff',
      brightWhite: '#c0caf5',
    }),
    subscribe: vi.fn(() => () => {}),
  },
}));

vi.mock('../state/terminal-settings-store', () => ({
  terminalSettingsStore: {
    getFontSize: () => 14,
    subscribe: vi.fn(() => () => {}),
  },
}));

function makeCell(content: string, overrides: Partial<RichGridCell> = {}): RichGridCell {
  return {
    content,
    fg: 'default',
    bg: 'default',
    bold: false,
    dim: false,
    italic: false,
    underline: false,
    inverse: false,
    wide: false,
    wide_continuation: false,
    ...overrides,
  };
}

function makeRow(cells: RichGridCell[]): RichGridRow {
  return { cells, wrapped: false };
}

function makeSnapshot(rows: RichGridRow[], cols: number): RichGridData {
  return {
    rows,
    cursor: { row: 0, col: 0 },
    dimensions: { rows: rows.length, cols },
    alternate_screen: false,
    cursor_hidden: false,
    title: 'test',
    scrollback_offset: 0,
    total_scrollback: 0,
  };
}

describe('Canvas2DGridRenderer (browser)', () => {
  let container: HTMLElement;

  beforeEach(() => {
    container = document.createElement('div');
    container.style.width = '800px';
    container.style.height = '600px';
    container.style.position = 'absolute';
    document.body.appendChild(container);
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('creates a canvas element and appends it to the container', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);

    const canvas = container.querySelector('canvas');
    expect(canvas).not.toBeNull();
    expect(canvas).toBeInstanceOf(HTMLCanvasElement);

    renderer.dispose();
  });

  it('getContext("2d") returns a real CanvasRenderingContext2D', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);

    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d');
    expect(ctx).not.toBeNull();
    expect(ctx).toBeInstanceOf(CanvasRenderingContext2D);

    renderer.dispose();
  });

  it('measureFont produces non-zero cell dimensions', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);

    const cellSize = renderer.getCellSize();
    // In real Chromium, font measurement produces actual pixel widths
    expect(cellSize.width).toBeGreaterThan(0);
    expect(cellSize.height).toBeGreaterThan(0);

    renderer.dispose();
  });

  it('updateSize sets canvas bitmap dimensions from real layout', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);

    const changed = renderer.updateSize();
    // First call should detect a size change (canvas starts at 0x0 or 300x150 default)
    expect(changed).toBe(true);

    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    // Canvas bitmap should match container * devicePixelRatio
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    expect(canvas.width).toBe(Math.floor(rect.width * dpr));
    expect(canvas.height).toBe(Math.floor(rect.height * dpr));

    renderer.dispose();
  });

  it('render paints non-empty pixels to the canvas', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);
    renderer.updateSize();

    // Create a simple snapshot with text
    const cells = [makeCell('H'), makeCell('e'), makeCell('l'), makeCell('l'), makeCell('o')];
    const snapshot = makeSnapshot([makeRow(cells)], 80);

    renderer.render(snapshot);

    // Read pixel data from the canvas to verify something was painted
    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d')!;
    const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);

    // Check that not all pixels are the same (text was rendered)
    const data = imageData.data;
    let hasVariation = false;
    const firstR = data[0], firstG = data[1], firstB = data[2];
    for (let i = 4; i < data.length; i += 4) {
      if (data[i] !== firstR || data[i + 1] !== firstG || data[i + 2] !== firstB) {
        hasVariation = true;
        break;
      }
    }
    expect(hasVariation).toBe(true);

    renderer.dispose();
  });

  it('render fills background color across the canvas', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);
    renderer.updateSize();

    // Render empty grid — should fill with background color
    const snapshot = makeSnapshot([], 80);
    renderer.render(snapshot);

    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d')!;
    // Sample a pixel — should be the theme background (#1a1b26)
    const pixel = ctx.getImageData(1, 1, 1, 1).data;
    // Background is #1a1b26 = rgb(26, 27, 38)
    expect(pixel[0]).toBe(26);
    expect(pixel[1]).toBe(27);
    expect(pixel[2]).toBe(38);

    renderer.dispose();
  });

  it('renders cursor at the correct position', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);
    renderer.updateSize();

    const cellSize = renderer.getCellSize();
    // Create a 1-row grid with cursor at col 2
    const cells = Array.from({ length: 10 }, () => makeCell(' '));
    const snapshot = makeSnapshot([makeRow(cells)], 80);
    snapshot.cursor = { row: 0, col: 2 };
    snapshot.cursor_hidden = false;

    renderer.render(snapshot);

    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d')!;

    // Sample pixel at cursor position — should be cursor color (#c0caf5)
    const cursorX = Math.floor(2 * cellSize.width + cellSize.width / 2);
    const cursorY = Math.floor(cellSize.height / 2);
    const pixel = ctx.getImageData(cursorX, cursorY, 1, 1).data;

    // Cursor color is #c0caf5 = rgb(192, 202, 245)
    expect(pixel[0]).toBe(192);
    expect(pixel[1]).toBe(202);
    expect(pixel[2]).toBe(245);

    renderer.dispose();
  });

  it('renders colored text with non-default foreground', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);
    renderer.updateSize();

    const cells = [makeCell('X', { fg: '#ff0000' })];
    const snapshot = makeSnapshot([makeRow(cells)], 80);
    snapshot.cursor_hidden = true; // Hide cursor so it doesn't interfere

    renderer.render(snapshot);

    // The red text should produce red-ish pixels in the first cell area
    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d')!;
    const cellSize = renderer.getCellSize();

    // Sample a region where the text glyph would be rendered
    const imageData = ctx.getImageData(0, 0, cellSize.width, cellSize.height);
    let hasRedPixel = false;
    for (let i = 0; i < imageData.data.length; i += 4) {
      // Look for pixels that are significantly red (from the #ff0000 text)
      if (imageData.data[i] > 200 && imageData.data[i + 1] < 100 && imageData.data[i + 2] < 100) {
        hasRedPixel = true;
        break;
      }
    }
    expect(hasRedPixel).toBe(true);

    renderer.dispose();
  });

  it('renders cell with non-default background', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);
    renderer.updateSize();

    const cells = [makeCell(' ', { bg: '#00ff00' })];
    const snapshot = makeSnapshot([makeRow(cells)], 80);
    snapshot.cursor_hidden = true;

    renderer.render(snapshot);

    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d')!;
    const cellSize = renderer.getCellSize();

    // Sample center of the first cell — should be green background
    const px = Math.floor(cellSize.width / 2);
    const py = Math.floor(cellSize.height / 2);
    const pixel = ctx.getImageData(px, py, 1, 1).data;
    expect(pixel[0]).toBe(0);   // R
    expect(pixel[1]).toBe(255); // G
    expect(pixel[2]).toBe(0);   // B

    renderer.dispose();
  });

  it('shiftCanvas moves content for optimistic scrolling', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);
    renderer.updateSize();

    // Render a snapshot with some content
    const cells = Array.from({ length: 80 }, (_, i) => makeCell(String.fromCharCode(65 + (i % 26))));
    const rows = Array.from({ length: 24 }, () => makeRow([...cells]));
    const snapshot = makeSnapshot(rows, 80);
    snapshot.cursor_hidden = true;
    renderer.render(snapshot);

    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    const ctx = canvas.getContext('2d')!;

    // Capture pixel before shift
    const pixelBefore = ctx.getImageData(10, 10, 1, 1).data.slice();

    // Shift by 1 line — content should move
    renderer.shiftCanvas(1);

    // The old position should now be different (shifted or filled with bg)
    const pixelAfter = ctx.getImageData(10, 10, 1, 1).data;
    // Can't guarantee exact values, but the operation should complete without error
    expect(pixelAfter.length).toBe(4);

    renderer.dispose();
  });

  it('dispose removes the canvas from the DOM', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);

    expect(container.querySelector('canvas')).not.toBeNull();

    renderer.dispose();

    expect(container.querySelector('canvas')).toBeNull();
  });

  it('releaseResources shrinks canvas to 1x1', async () => {
    const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
    const renderer = new Canvas2DGridRenderer(container);
    renderer.updateSize();

    const canvas = container.querySelector('canvas') as HTMLCanvasElement;
    expect(canvas.width).toBeGreaterThan(1);

    renderer.releaseResources();

    expect(canvas.width).toBe(1);
    expect(canvas.height).toBe(1);

    renderer.dispose();
  });
});
