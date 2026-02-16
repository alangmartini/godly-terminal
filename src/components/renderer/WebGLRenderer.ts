import { ColorCache } from './ColorCache';
import { GlyphAtlas } from './GlyphAtlas';
import { CellDataEncoder, Selection } from './CellDataEncoder';
import { VERTEX_SHADER, FRAGMENT_SHADER } from './shaders';
import type { RichGridData, TerminalTheme } from '../TerminalRenderer';
import { perfTracer } from '../../utils/PerfTracer';

function compileShader(gl: WebGL2RenderingContext, type: number, source: string): WebGLShader {
  const shader = gl.createShader(type);
  if (!shader) throw new Error('Failed to create shader');
  gl.shaderSource(shader, source);
  gl.compileShader(shader);
  if (!gl.getShaderParameter(shader, gl.COMPILE_STATUS)) {
    const info = gl.getShaderInfoLog(shader);
    gl.deleteShader(shader);
    throw new Error(`Shader compile error: ${info}`);
  }
  return shader;
}

function linkProgram(gl: WebGL2RenderingContext, vs: WebGLShader, fs: WebGLShader): WebGLProgram {
  const program = gl.createProgram();
  if (!program) throw new Error('Failed to create program');
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const info = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    throw new Error(`Program link error: ${info}`);
  }
  // Shaders can be detached after linking
  gl.detachShader(program, vs);
  gl.detachShader(program, fs);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  return program;
}

/** Unpack a 0xRRGGBBFF packed uint32 into a [r, g, b, a] float array (0..1). */
function packedToVec4(packed: number): [number, number, number, number] {
  return [
    ((packed >>> 24) & 0xFF) / 255,
    ((packed >>> 16) & 0xFF) / 255,
    ((packed >>> 8) & 0xFF) / 255,
    (packed & 0xFF) / 255,
  ];
}

const UNIFORM_NAMES = [
  'u_resolution', 'u_cellSize', 'u_gridCols', 'u_gridRows', 'u_atlasSize',
  'u_cellData', 'u_atlas',
  'u_cursorRow', 'u_cursorCol', 'u_cursorVisible', 'u_cursorColor',
  'u_selColor',
] as const;

type UniformName = typeof UNIFORM_NAMES[number];

export class WebGLRenderer {
  private gl: WebGL2RenderingContext;
  private program: WebGLProgram;
  private vao: WebGLVertexArrayObject;
  private cellDataTexture: WebGLTexture;

  private atlas: GlyphAtlas;
  private colorCache: ColorCache;
  private encoder: CellDataEncoder;

  private uniforms: Record<UniformName, WebGLUniformLocation>;

  private cellWidth = 0;
  private cellHeight = 0;
  private dpr: number;

  private fontFamily: string;
  private fontSize: number;

  constructor(gl: WebGL2RenderingContext, fontFamily: string, fontSize: number, dpr: number) {
    this.gl = gl;
    this.fontFamily = fontFamily;
    this.fontSize = fontSize;
    this.dpr = dpr;

    this.colorCache = new ColorCache();
    this.atlas = new GlyphAtlas(fontFamily, fontSize, dpr);
    this.encoder = new CellDataEncoder();

    // Compile and link shaders
    console.log('[WebGLRenderer] Compiling shaders...');
    const vs = compileShader(gl, gl.VERTEX_SHADER, VERTEX_SHADER);
    const fs = compileShader(gl, gl.FRAGMENT_SHADER, FRAGMENT_SHADER);
    this.program = linkProgram(gl, vs, fs);
    console.log('[WebGLRenderer] Shaders compiled and linked');

    // Get all uniform locations
    this.uniforms = {} as Record<UniformName, WebGLUniformLocation>;
    for (const name of UNIFORM_NAMES) {
      const loc = gl.getUniformLocation(this.program, name);
      if (!loc) throw new Error(`Uniform '${name}' not found in shader program`);
      this.uniforms[name] = loc;
    }
    console.log('[WebGLRenderer] All uniforms located:', UNIFORM_NAMES.join(', '));

    // Create empty VAO (vertex data generated in shader from gl_VertexID/gl_InstanceID)
    const vao = gl.createVertexArray();
    if (!vao) throw new Error('Failed to create VAO');
    this.vao = vao;

    // Create cell data texture (RGBA32UI, integer texture)
    const cellDataTex = gl.createTexture();
    if (!cellDataTex) throw new Error('Failed to create cell data texture');
    this.cellDataTexture = cellDataTex;
    gl.bindTexture(gl.TEXTURE_2D, this.cellDataTexture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);

    // Pre-populate atlas with ASCII glyphs
    this.atlas.populateAscii();

    // Measure font to get cell dimensions
    const metrics = this.measureFont();
    this.cellWidth = metrics.cellWidth;
    this.cellHeight = metrics.cellHeight;

    // Initial GL state
    gl.disable(gl.BLEND);
    gl.clearColor(0, 0, 0, 1);
  }

  paint(
    snapshot: RichGridData,
    theme: TerminalTheme,
    selection: Selection | null,
    cursorVisible: boolean,
  ): void {
    const gl = this.gl;
    const { rows: gridRows, cols: gridCols } = snapshot.dimensions;

    // Encode cell data
    perfTracer.mark('encode_start');
    const cellData = this.encoder.encode(snapshot, theme, this.atlas, this.colorCache, selection);
    perfTracer.measure('cell_data_encode', 'encode_start');

    // Upload cell data to texture
    perfTracer.mark('gpu_upload_start');
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, this.cellDataTexture);
    gl.texImage2D(
      gl.TEXTURE_2D, 0, gl.RGBA32UI,
      gridCols, gridRows, 0,
      gl.RGBA_INTEGER, gl.UNSIGNED_INT,
      cellData,
    );

    // Upload atlas if dirty
    gl.activeTexture(gl.TEXTURE1);
    this.atlas.uploadToGL(gl);

    // Use program and bind VAO
    gl.useProgram(this.program);
    gl.bindVertexArray(this.vao);

    // Set uniforms
    gl.uniform2f(this.uniforms.u_resolution, gl.canvas.width, gl.canvas.height);
    gl.uniform2f(this.uniforms.u_cellSize, this.cellWidth, this.cellHeight);
    gl.uniform1i(this.uniforms.u_gridCols, gridCols);
    gl.uniform1i(this.uniforms.u_gridRows, gridRows);
    gl.uniform2f(this.uniforms.u_atlasSize, this.atlas.width, this.atlas.height);

    // Cursor uniforms
    gl.uniform1i(this.uniforms.u_cursorRow, snapshot.cursor.row);
    gl.uniform1i(this.uniforms.u_cursorCol, snapshot.cursor.col);
    gl.uniform1f(this.uniforms.u_cursorVisible, cursorVisible && !snapshot.cursor_hidden && snapshot.scrollback_offset === 0 ? 1.0 : 0.0);
    const cursorColor = packedToVec4(this.colorCache.parse(theme.cursor));
    gl.uniform4f(this.uniforms.u_cursorColor, cursorColor[0], cursorColor[1], cursorColor[2], cursorColor[3]);

    // Selection color uniform
    const selColor = packedToVec4(this.colorCache.parse(theme.selectionBackground));
    gl.uniform4f(this.uniforms.u_selColor, selColor[0], selColor[1], selColor[2], selColor[3]);

    // Bind textures to sampler units
    gl.uniform1i(this.uniforms.u_cellData, 0);
    gl.uniform1i(this.uniforms.u_atlas, 1);

    // Draw: 6 vertices per cell (2 triangles), one instance per cell
    gl.drawArraysInstanced(gl.TRIANGLES, 0, 6, gridCols * gridRows);
    perfTracer.measure('gpu_upload_and_draw', 'gpu_upload_start');
  }

  resize(width: number, height: number, dpr: number): void {
    const gl = this.gl;
    gl.viewport(0, 0, width, height);

    if (dpr !== this.dpr) {
      this.dpr = dpr;
      this.atlas.invalidate(dpr);
      this.atlas.populateAscii();
      const metrics = this.measureFont();
      this.cellWidth = metrics.cellWidth;
      this.cellHeight = metrics.cellHeight;
    }
  }

  measureFont(): { cellWidth: number; cellHeight: number } {
    const scaledSize = this.fontSize * this.dpr;
    const canvas = new OffscreenCanvas(64, 64);
    const ctx = canvas.getContext('2d')!;
    ctx.font = `${scaledSize}px ${this.fontFamily}`;
    const cellWidth = Math.ceil(ctx.measureText('M').width);
    const cellHeight = Math.ceil(scaledSize * 1.2);
    return { cellWidth, cellHeight };
  }

  dispose(): void {
    const gl = this.gl;
    gl.deleteProgram(this.program);
    gl.deleteVertexArray(this.vao);
    gl.deleteTexture(this.cellDataTexture);
    this.atlas.dispose(gl);
  }
}
