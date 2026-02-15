export interface GlyphEntry {
  x: number;  // x position in atlas pixels
  y: number;  // y position in atlas pixels
  w: number;  // glyph width in atlas pixels
  h: number;  // glyph height in atlas pixels
}

const INITIAL_SIZE = 1024;
const MAX_SIZE = 4096;
const LINE_HEIGHT_FACTOR = 1.2;
const GLYPH_PAD = 1; // 1px padding between glyphs to prevent texture bleeding

export class GlyphAtlas {
  private canvas: OffscreenCanvas;
  private ctx: OffscreenCanvasRenderingContext2D;
  private glyphs = new Map<string, GlyphEntry>();
  private texture: WebGLTexture | null = null;
  private _dirty = true;

  // Shelf packing state
  private shelfX = 0;
  private shelfY = 0;
  private shelfHeight = 0;
  private atlasWidth: number;
  private atlasHeight: number;

  // Cell metrics (computed once, in atlas pixels)
  private cellWidth = 0;
  private cellHeight = 0;

  private fontFamily: string;
  private fontSize: number;
  private dpr: number;

  constructor(fontFamily: string, fontSize: number, dpr: number) {
    this.fontFamily = fontFamily;
    this.fontSize = fontSize;
    this.dpr = dpr;
    this.atlasWidth = INITIAL_SIZE;
    this.atlasHeight = INITIAL_SIZE;
    this.canvas = new OffscreenCanvas(this.atlasWidth, this.atlasHeight);
    this.ctx = this.canvas.getContext('2d', { willReadFrequently: false })!;
    this.computeCellMetrics();
  }

  get dirty(): boolean { return this._dirty; }
  get width(): number { return this.atlasWidth; }
  get height(): number { return this.atlasHeight; }

  /** Get or create a glyph entry. Returns atlas pixel coords. */
  getGlyph(char: string, bold: boolean, italic: boolean): GlyphEntry {
    const key = `${char}|${bold ? 1 : 0}|${italic ? 1 : 0}`;
    const existing = this.glyphs.get(key);
    if (existing) return existing;
    return this.rasterize(char, bold, italic, key);
  }

  /** Upload atlas to WebGL texture. Creates texture if needed. */
  uploadToGL(gl: WebGL2RenderingContext): WebGLTexture {
    if (!this.texture) {
      this.texture = gl.createTexture()!;
      gl.bindTexture(gl.TEXTURE_2D, this.texture);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    } else {
      gl.bindTexture(gl.TEXTURE_2D, this.texture);
    }

    if (this._dirty) {
      gl.texImage2D(
        gl.TEXTURE_2D, 0, gl.RGBA,
        gl.RGBA, gl.UNSIGNED_BYTE,
        this.canvas,
      );
      this._dirty = false;
    }

    return this.texture;
  }

  /** Pre-populate ASCII printable range (32-126) in regular, bold, italic. */
  populateAscii(): void {
    for (let code = 32; code <= 126; code++) {
      const ch = String.fromCharCode(code);
      this.getGlyph(ch, false, false);
      this.getGlyph(ch, true, false);
      this.getGlyph(ch, false, true);
    }
  }

  /** Invalidate entire atlas (e.g., on DPR change). Clears everything. */
  invalidate(newDpr: number): void {
    this.dpr = newDpr;
    this.glyphs.clear();
    this.shelfX = 0;
    this.shelfY = 0;
    this.shelfHeight = 0;
    this.atlasWidth = INITIAL_SIZE;
    this.atlasHeight = INITIAL_SIZE;
    this.canvas = new OffscreenCanvas(this.atlasWidth, this.atlasHeight);
    this.ctx = this.canvas.getContext('2d', { willReadFrequently: false })!;
    this.computeCellMetrics();
    this._dirty = true;
    // Old texture will be replaced on next uploadToGL call
  }

  /** Dispose GL resources. */
  dispose(gl: WebGL2RenderingContext): void {
    if (this.texture) {
      gl.deleteTexture(this.texture);
      this.texture = null;
    }
  }

  private computeCellMetrics(): void {
    const scaledSize = this.fontSize * this.dpr;
    this.cellHeight = Math.ceil(scaledSize * LINE_HEIGHT_FACTOR);
    // Measure 'M' width for monospace cell width
    this.ctx.font = `${scaledSize}px ${this.fontFamily}`;
    this.cellWidth = Math.ceil(this.ctx.measureText('M').width);
  }

  private rasterize(char: string, bold: boolean, italic: boolean, key: string): GlyphEntry {
    const scaledSize = this.fontSize * this.dpr;
    const w = this.cellWidth;
    const h = this.cellHeight;

    // Ensure space in current shelf
    this.ensureSpace(w, h);

    const x = this.shelfX;
    const y = this.shelfY;

    // Build font string
    const weight = bold ? 'bold' : 'normal';
    const style = italic ? 'italic' : 'normal';
    const fontStr = `${style} ${weight} ${scaledSize}px ${this.fontFamily}`;

    // Clear the glyph area and render white text
    this.ctx.clearRect(x, y, w, h);
    this.ctx.font = fontStr;
    this.ctx.textBaseline = 'top';
    this.ctx.fillStyle = '#ffffff';
    // Offset baseline down a bit for vertical centering
    const baselineOffset = Math.round((h - scaledSize) / 2);
    this.ctx.fillText(char, x, y + baselineOffset);

    // Advance shelf cursor with padding
    this.shelfX += w + GLYPH_PAD;
    if (h + GLYPH_PAD > this.shelfHeight) {
      this.shelfHeight = h + GLYPH_PAD;
    }

    const entry: GlyphEntry = { x, y, w, h };
    this.glyphs.set(key, entry);
    this._dirty = true;
    return entry;
  }

  private ensureSpace(glyphW: number, glyphH: number): void {
    // Check if glyph fits in current shelf horizontally (with padding)
    if (this.shelfX + glyphW + GLYPH_PAD > this.atlasWidth) {
      // Start a new shelf
      this.shelfY += this.shelfHeight;
      this.shelfX = 0;
      this.shelfHeight = 0;
    }

    // Check if we have vertical space (with padding)
    const neededHeight = this.shelfY + Math.max(this.shelfHeight, glyphH + GLYPH_PAD);
    if (neededHeight > this.atlasHeight) {
      this.grow(neededHeight);
    }
  }

  private grow(neededHeight: number): void {
    let newHeight = this.atlasHeight;
    while (newHeight < neededHeight && newHeight < MAX_SIZE) {
      newHeight *= 2;
    }
    if (newHeight > MAX_SIZE) newHeight = MAX_SIZE;
    if (newHeight <= this.atlasHeight) return; // Can't grow further

    const newCanvas = new OffscreenCanvas(this.atlasWidth, newHeight);
    const newCtx = newCanvas.getContext('2d', { willReadFrequently: false })!;
    // Copy old atlas content
    newCtx.drawImage(this.canvas, 0, 0);

    this.canvas = newCanvas;
    this.ctx = newCtx;
    this.atlasHeight = newHeight;
    this._dirty = true;
  }
}
