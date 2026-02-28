/**
 * Canvas2DGridRenderer cursor focus browser tests.
 *
 * Bug #425: When switching panes with a hotkey, the blinking cursor stays
 * in the previously focused pane. Both panes render identical blinking block
 * cursors because Canvas2DGridRenderer has no concept of focus state.
 *
 * These tests verify that the cursor rendered in an unfocused pane is visually
 * distinct from the cursor in the focused pane (e.g., outline/hollow cursor
 * vs solid block). Requires real Chromium for Canvas2D pixel inspection.
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

/** Create a container, renderer, and snapshot with cursor at a known position. */
async function createRendererWithCursor() {
  const container = document.createElement('div');
  container.style.width = '400px';
  container.style.height = '300px';
  container.style.position = 'absolute';
  document.body.appendChild(container);

  const { Canvas2DGridRenderer } = await import('./Canvas2DGridRenderer');
  const renderer = new Canvas2DGridRenderer(container);
  renderer.updateSize();

  // Create snapshot with cursor at (0, 5) — middle of the row for clean pixel sampling
  const cells = Array.from({ length: 80 }, () => makeCell(' '));
  const rows = Array.from({ length: 24 }, () => makeRow([...cells]));
  const snapshot = makeSnapshot(rows, 80);
  snapshot.cursor = { row: 0, col: 5 };
  snapshot.cursor_hidden = false;

  return { container, renderer, snapshot };
}

/** Sample a pixel at the center of the cursor cell. */
function sampleCursorCenter(
  container: HTMLElement,
  cellWidth: number,
  cellHeight: number,
  cursorCol: number,
  cursorRow: number,
): [number, number, number, number] {
  const canvas = container.querySelector('canvas') as HTMLCanvasElement;
  const ctx = canvas.getContext('2d')!;
  const x = Math.floor(cursorCol * cellWidth + cellWidth / 2);
  const y = Math.floor(cursorRow * cellHeight + cellHeight / 2);
  const pixel = ctx.getImageData(x, y, 1, 1).data;
  return [pixel[0], pixel[1], pixel[2], pixel[3]];
}

/** Check if a pixel matches the cursor color (#c0caf5 = rgb(192, 202, 245)). */
function isCursorColor(r: number, g: number, b: number): boolean {
  return r === 192 && g === 202 && b === 245;
}

/** Check if a pixel matches the background color (#1a1b26 = rgb(26, 27, 38)). */
function isBackgroundColor(r: number, g: number, b: number): boolean {
  return r === 26 && g === 27 && b === 38;
}

describe('Canvas2DGridRenderer cursor focus distinction (Bug #425)', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('focused pane renders a solid block cursor', async () => {
    const { container, renderer, snapshot } = await createRendererWithCursor();

    // Signal focused state if the API exists, otherwise just render
    if (typeof (renderer as any).setFocused === 'function') {
      (renderer as any).setFocused(true);
    }
    renderer.render(snapshot);

    const cellSize = renderer.getCellSize();
    const [r, g, b] = sampleCursorCenter(container, cellSize.width, cellSize.height, 5, 0);

    // Focused cursor should be a solid block — cursor color at center
    expect(isCursorColor(r, g, b)).toBe(true);

    renderer.dispose();
  });

  it('unfocused pane cursor center should NOT be solid cursor color', async () => {
    // Bug #425: The unfocused pane should render a different cursor style
    // (e.g., outline/hollow cursor where the center shows background color,
    // or no cursor at all). Currently both panes render identical solid blocks.
    const { container, renderer, snapshot } = await createRendererWithCursor();

    // Signal unfocused state
    if (typeof (renderer as any).setFocused === 'function') {
      (renderer as any).setFocused(false);
    }
    renderer.render(snapshot);

    const cellSize = renderer.getCellSize();
    const [r, g, b] = sampleCursorCenter(container, cellSize.width, cellSize.height, 5, 0);

    // The center of an unfocused cursor should NOT be the solid cursor color.
    // An outline/hollow cursor would show background color at its center.
    // A hidden cursor would also show background color.
    // Either way, it must differ from the focused solid block.
    expect(isCursorColor(r, g, b)).toBe(false);

    renderer.dispose();
  });

  it('focused and unfocused cursors produce different pixel output', async () => {
    // Bug #425: Both panes currently render identical cursors.
    // After fix, the cursor pixels at the center of the cell must differ.
    const focused = await createRendererWithCursor();
    const unfocused = await createRendererWithCursor();

    // Set focus states
    if (typeof (focused.renderer as any).setFocused === 'function') {
      (focused.renderer as any).setFocused(true);
    }
    if (typeof (unfocused.renderer as any).setFocused === 'function') {
      (unfocused.renderer as any).setFocused(false);
    }

    focused.renderer.render(focused.snapshot);
    unfocused.renderer.render(unfocused.snapshot);

    const focusedCellSize = focused.renderer.getCellSize();
    const unfocusedCellSize = unfocused.renderer.getCellSize();

    const [fr, fg, fb] = sampleCursorCenter(
      focused.container, focusedCellSize.width, focusedCellSize.height, 5, 0,
    );
    const [ur, ug, ub] = sampleCursorCenter(
      unfocused.container, unfocusedCellSize.width, unfocusedCellSize.height, 5, 0,
    );

    // The focused cursor center should be the cursor color (solid block)
    expect(isCursorColor(fr, fg, fb)).toBe(true);

    // The unfocused cursor center must differ from the focused cursor center.
    // Whether it's background-colored (outline cursor), dimmed, or hidden —
    // the pixels must be visually distinct.
    const pixelsMatch = fr === ur && fg === ug && fb === ub;
    expect(pixelsMatch).toBe(false);

    focused.renderer.dispose();
    unfocused.renderer.dispose();
  });

  it('unfocused pane should not have a blinking cursor timer', async () => {
    // Bug #425: Both panes run independent cursorBlinkTimer intervals.
    // The unfocused pane's blink timer creates the illusion that it still has
    // active focus. After fix, the unfocused pane should either:
    // - Stop the blink timer entirely (static cursor), or
    // - Never start it when unfocused
    const { renderer } = await createRendererWithCursor();

    if (typeof (renderer as any).setFocused === 'function') {
      (renderer as any).setFocused(false);
    }

    // Access the private cursorBlinkTimer — it should be null for unfocused panes
    const blinkTimer = (renderer as any).cursorBlinkTimer;
    expect(blinkTimer).toBeNull();

    renderer.dispose();
  });
});
