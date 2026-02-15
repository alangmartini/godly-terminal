export class ColorCache {
  private cache = new Map<string, number>();

  /** Parse a CSS color string to a packed uint32 (0xRRGGBBFF). */
  parse(color: string): number {
    const cached = this.cache.get(color);
    if (cached !== undefined) return cached;
    const val = this._parse(color);
    this.cache.set(color, val);
    return val;
  }

  /** Dim a color by reducing RGB channels by ~33%. */
  dim(packed: number): number {
    const r = (packed >>> 24) & 0xFF;
    const g = (packed >>> 16) & 0xFF;
    const b = (packed >>> 8) & 0xFF;
    const a = packed & 0xFF;
    return ((Math.round(r * 0.67) << 24) | (Math.round(g * 0.67) << 16) | (Math.round(b * 0.67) << 8) | a) >>> 0;
  }

  clear() { this.cache.clear(); }

  private _parse(color: string): number {
    // Handle #RRGGBB and #RGB
    if (color.startsWith('#')) {
      let hex = color.slice(1);
      if (hex.length === 3) hex = hex[0]+hex[0]+hex[1]+hex[1]+hex[2]+hex[2];
      const r = parseInt(hex.slice(0,2), 16);
      const g = parseInt(hex.slice(2,4), 16);
      const b = parseInt(hex.slice(4,6), 16);
      return ((r << 24) | (g << 16) | (b << 8) | 0xFF) >>> 0;
    }
    // Handle rgb(r,g,b)
    const rgbMatch = color.match(/rgb\(\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\)/);
    if (rgbMatch) {
      const r = parseInt(rgbMatch[1]);
      const g = parseInt(rgbMatch[2]);
      const b = parseInt(rgbMatch[3]);
      return ((r << 24) | (g << 16) | (b << 8) | 0xFF) >>> 0;
    }
    // Fallback: white
    return 0xFFFFFFFF;
  }
}
