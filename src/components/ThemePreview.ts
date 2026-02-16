/**
 * Self-contained mini terminal preview rendered onto a canvas.
 * Shows how a theme's terminal colors look in a realistic layout.
 */

import type { ThemeDefinition } from '../themes/types';

/**
 * Create a canvas element that draws a miniature terminal preview
 * using the given theme's terminal color palette.
 */
export function createThemePreview(
  theme: ThemeDefinition,
  width: number,
  height: number,
): HTMLCanvasElement {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;

  const ctx = canvas.getContext('2d');
  if (!ctx) return canvas;

  const t = theme.terminal;
  const font = "11px 'Cascadia Code', Consolas, monospace";
  const lineHeight = 16;
  const startX = 8;
  let y = 16;

  // Background fill
  ctx.fillStyle = t.background;
  ctx.fillRect(0, 0, width, height);

  ctx.font = font;
  ctx.textBaseline = 'alphabetic';

  // Line 1:  $ git status
  drawSegments(ctx, startX, y, [
    { text: '$', color: t.green },
    { text: ' git status', color: t.foreground },
  ]);
  y += lineHeight;

  // Line 2:  On branch main
  drawSegments(ctx, startX, y, [
    { text: 'On branch ', color: t.foreground },
    { text: 'main', color: t.cyan },
  ]);
  y += lineHeight;

  // Line 3:  Changes:
  drawSegments(ctx, startX, y, [
    { text: 'Changes:', color: t.yellow },
  ]);
  y += lineHeight;

  // Line 4:    modified: src/app.ts
  drawSegments(ctx, startX, y, [
    { text: '  modified: ', color: t.foreground },
    { text: 'src/app.ts', color: t.red },
  ]);
  y += lineHeight;

  // Line 5:  $ + cursor block
  const dollarWidth = drawSegments(ctx, startX, y, [
    { text: '$ ', color: t.green },
  ]);
  // Draw cursor block
  ctx.fillStyle = t.cursor;
  ctx.fillRect(startX + dollarWidth, y - 10, 7, 13);

  return canvas;
}

interface TextSegment {
  text: string;
  color: string;
}

/** Draw text segments sequentially, returning the total width drawn. */
function drawSegments(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  segments: TextSegment[],
): number {
  let offsetX = 0;
  for (const seg of segments) {
    ctx.fillStyle = seg.color;
    ctx.fillText(seg.text, x + offsetX, y);
    offsetX += ctx.measureText(seg.text).width;
  }
  return offsetX;
}
