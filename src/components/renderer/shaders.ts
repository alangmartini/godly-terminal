export const VERTEX_SHADER = `#version 300 es
precision highp float;
precision highp usampler2D;

uniform vec2 u_resolution;    // canvas pixel size
uniform vec2 u_cellSize;      // cell width, height in pixels
uniform int u_gridCols;
uniform int u_gridRows;
uniform vec2 u_atlasSize;     // atlas texture size in pixels

uniform usampler2D u_cellData;

out vec2 v_uv;                // UV within glyph in atlas (0..1 of glyph rect)
flat out vec4 v_fgColor;
flat out vec4 v_bgColor;
flat out float v_underline;
flat out float v_wideCont;
flat out float v_selected;
flat out vec4 v_glyphRect;    // x, y, w, h in atlas pixels
flat out float v_cellRow;
flat out float v_cellCol;

void main() {
  int cellIndex = gl_InstanceID;
  int col = cellIndex % u_gridCols;
  int row = cellIndex / u_gridCols;

  v_cellRow = float(row);
  v_cellCol = float(col);

  // Skip rows beyond grid
  if (row >= u_gridRows) { gl_Position = vec4(0.0); return; }

  // Read cell data
  uvec4 data = texelFetch(u_cellData, ivec2(col, row), 0);
  uint fgPacked = data.r;
  uint bgPacked = data.g;
  uint atlasXY = data.b;
  uint glyphInfo = data.a;

  // Unpack colors (0xRRGGBBFF -> vec4)
  v_fgColor = vec4(
    float((fgPacked >> 24u) & 0xFFu) / 255.0,
    float((fgPacked >> 16u) & 0xFFu) / 255.0,
    float((fgPacked >> 8u) & 0xFFu) / 255.0,
    float(fgPacked & 0xFFu) / 255.0
  );
  v_bgColor = vec4(
    float((bgPacked >> 24u) & 0xFFu) / 255.0,
    float((bgPacked >> 16u) & 0xFFu) / 255.0,
    float((bgPacked >> 8u) & 0xFFu) / 255.0,
    float(bgPacked & 0xFFu) / 255.0
  );

  // Unpack atlas coords
  float atlasX = float(atlasXY & 0xFFFFu);
  float atlasY = float((atlasXY >> 16u) & 0xFFFFu);
  float glyphW = float(glyphInfo & 0xFFu);
  float glyphH = float((glyphInfo >> 8u) & 0xFFu);
  uint flags = (glyphInfo >> 16u) & 0xFFu;

  v_underline = ((flags & 4u) != 0u) ? 1.0 : 0.0;
  v_wideCont = ((flags & 64u) != 0u) ? 1.0 : 0.0;
  v_selected = ((flags & 128u) != 0u) ? 1.0 : 0.0;
  v_glyphRect = vec4(atlasX, atlasY, glyphW, glyphH);

  // Generate quad vertices from gl_VertexID (0-5 for 2 triangles)
  // 0: TL, 1: TR, 2: BL, 3: BL, 4: TR, 5: BR
  int vid = gl_VertexID;
  // Use wide cell width for wide chars
  float cellW = ((flags & 32u) != 0u) ? u_cellSize.x * 2.0 : u_cellSize.x;

  vec2 corner;
  if (vid == 0) corner = vec2(0.0, 0.0);       // TL
  else if (vid == 1) corner = vec2(1.0, 0.0);   // TR
  else if (vid == 2) corner = vec2(0.0, 1.0);   // BL
  else if (vid == 3) corner = vec2(0.0, 1.0);   // BL
  else if (vid == 4) corner = vec2(1.0, 0.0);   // TR
  else corner = vec2(1.0, 1.0);                  // BR

  v_uv = corner;

  vec2 cellPos = vec2(float(col) * u_cellSize.x, float(row) * u_cellSize.y);
  vec2 pos = cellPos + corner * vec2(cellW, u_cellSize.y);

  // Convert to clip space (-1..1)
  vec2 clipPos = (pos / u_resolution) * 2.0 - 1.0;
  clipPos.y = -clipPos.y; // flip Y
  gl_Position = vec4(clipPos, 0.0, 1.0);
}
`;

export const FRAGMENT_SHADER = `#version 300 es
precision highp float;

uniform sampler2D u_atlas;
uniform vec2 u_atlasSize;
uniform vec2 u_cellSize;

// Cursor
uniform int u_cursorRow;
uniform int u_cursorCol;
uniform float u_cursorVisible;
uniform vec4 u_cursorColor;

// Selection highlight color
uniform vec4 u_selColor;

in vec2 v_uv;
flat in vec4 v_fgColor;
flat in vec4 v_bgColor;
flat in float v_underline;
flat in float v_wideCont;
flat in float v_selected;
flat in vec4 v_glyphRect; // x, y, w, h in atlas pixels
flat in float v_cellRow;
flat in float v_cellCol;

out vec4 fragColor;

void main() {
  // Skip wide continuation cells
  if (v_wideCont > 0.5) { discard; }

  // Start with background color
  vec4 color = v_bgColor;

  // Selection overlay
  if (v_selected > 0.5) {
    color = u_selColor;
  }

  // Sample glyph from atlas
  if (v_glyphRect.z > 0.0 && v_glyphRect.w > 0.0) {
    vec2 glyphUV = (v_glyphRect.xy + v_uv * v_glyphRect.zw) / u_atlasSize;
    // Use max(r,g,b) to handle Windows ClearType subpixel rendering,
    // which writes different values per RGB channel
    vec4 texel = texture(u_atlas, glyphUV);
    float alpha = max(texel.r, max(texel.g, texel.b));
    vec3 textColor = v_selected > 0.5 ? vec3(1.0) : v_fgColor.rgb;
    color = vec4(mix(color.rgb, textColor, alpha), 1.0);
  }

  // Underline: bottom 1px of cell
  if (v_underline > 0.5 && v_uv.y > (1.0 - 1.0 / u_cellSize.y)) {
    color = vec4(v_fgColor.rgb, 1.0);
  }

  // Cursor: block overlay with alpha blending
  if (u_cursorVisible > 0.5 &&
      int(v_cellRow) == u_cursorRow &&
      int(v_cellCol) == u_cursorCol) {
    color = vec4(mix(color.rgb, u_cursorColor.rgb, 0.7), 1.0);
  }

  fragColor = color;
}
`;
