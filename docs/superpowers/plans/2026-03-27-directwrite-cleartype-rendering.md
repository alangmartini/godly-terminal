# DirectWrite ClearType Rendering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the existing DirectWrite ClearType rasterizer into the pixel rendering pipeline so terminal text uses Windows-native subpixel rendering.

**Architecture:** The `DirectWriteRasterizer` already exists and produces ClearType 3x1 RGB bitmaps. We need to: (1) make it implement the `GlyphRasterizer` trait so it's interchangeable with `SwashRasterizer`, (2) use it on Windows in `app.rs`, and (3) enable the pixel renderer by default. The existing `blit_subpixel()` compositing path will automatically activate once glyphs arrive as `SubpixelRgb`.

**Tech Stack:** Rust, Windows DirectWrite API (`windows` crate 0.58), iced framework

**Refs:** GitHub Issue #841

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `terminal-surface/src/directwrite_rasterizer.rs` | Modify | Add `GlyphRasterizer` trait impl, bold/italic font face support |
| `terminal-surface/src/lib.rs` | No change | Already exports `directwrite_rasterizer` under `#[cfg(windows)]` |
| `iced-shell/src/app.rs` | Modify | Use DirectWrite on Windows, enable pixel renderer, load font weights |
| `terminal-surface/src/glyph_rasterizer.rs` | No change | Trait already supports `SubpixelRgb` format |
| `terminal-surface/src/pixel_renderer.rs` | No change | `blit_subpixel()` already handles `SubpixelRgb` glyphs |

All paths relative to `src-tauri/native/`.

---

### Task 1: Implement `GlyphRasterizer` trait for `DirectWriteRasterizer`

**Files:**
- Modify: `terminal-surface/src/directwrite_rasterizer.rs`

The `DirectWriteRasterizer` has its own API (`rasterize_glyph`, `load_system_font`, `measure_font`) but doesn't implement the `GlyphRasterizer` trait that the pixel renderer requires. We need to bridge the gap.

The trait requires:
```rust
trait GlyphRasterizer {
    fn rasterize(&mut self, ch: char, font_size_px: f32, bold: bool, italic: bool) -> Option<RasterizedGlyph>;
    fn measure(&mut self, font_size_px: f32) -> MeasuredFontMetrics;
    fn has_glyph(&self, ch: char) -> bool;
    fn load_font(&mut self, data: &[u8], index: u32) -> bool;
}
```

The DirectWrite rasterizer loads by family name (`load_system_font`), not by bytes. We need `load_font` to work with embedded font data as a fallback, but the primary path will be system font loading. We also need bold/italic support via additional font faces.

- [ ] **Step 1: Add bold/italic font face fields and loading**

Add fields for bold, italic, and bold-italic font faces to `DirectWriteRasterizer`. Add a method to load a specific weight/style variant.

In `terminal-surface/src/directwrite_rasterizer.rs`, add these fields to the struct:

```rust
pub struct DirectWriteRasterizer {
    factory: IDWriteFactory,
    font_face: Option<IDWriteFontFace>,
    bold_face: Option<IDWriteFontFace>,
    italic_face: Option<IDWriteFontFace>,
    bold_italic_face: Option<IDWriteFontFace>,
    rendering_params: IDWriteRenderingParams,
    scale_factor: f32,
    /// Cached font family name for has_glyph lookups.
    font_family_name: String,
}
```

Update `new()` to initialize the new fields to `None` and `font_family_name` to `String::new()`.

Update `load_system_font` to also load the bold, italic, and bold-italic faces:

```rust
pub fn load_system_font(&mut self, family_name: &str) -> windows::core::Result<()> {
    unsafe {
        let mut collection: Option<IDWriteFontCollection> = None;
        self.factory
            .GetSystemFontCollection(&mut collection, false)?;
        let collection = collection.ok_or(windows::core::Error::from(E_FAIL))?;

        let mut index = 0u32;
        let mut exists = BOOL::default();
        let family_wide: Vec<u16> = family_name
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let family_pcwstr = PCWSTR(family_wide.as_ptr());
        collection.FindFamilyName(family_pcwstr, &mut index, &mut exists)?;

        if !exists.as_bool() {
            return Err(windows::core::Error::from(E_FAIL));
        }

        let font_family = collection.GetFontFamily(index)?;

        // Regular
        let regular = font_family.GetFirstMatchingFont(
            DWRITE_FONT_WEIGHT_REGULAR,
            DWRITE_FONT_STRETCH_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
        )?;
        self.font_face = Some(regular.CreateFontFace()?);

        // Bold (optional — fall back to regular if not found)
        self.bold_face = font_family
            .GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
            )
            .ok()
            .and_then(|f| f.CreateFontFace().ok());

        // Italic (optional)
        self.italic_face = font_family
            .GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_REGULAR,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_ITALIC,
            )
            .ok()
            .and_then(|f| f.CreateFontFace().ok());

        // Bold-Italic (optional)
        self.bold_italic_face = font_family
            .GetFirstMatchingFont(
                DWRITE_FONT_WEIGHT_BOLD,
                DWRITE_FONT_STRETCH_NORMAL,
                DWRITE_FONT_STYLE_ITALIC,
            )
            .ok()
            .and_then(|f| f.CreateFontFace().ok());

        self.font_family_name = family_name.to_string();
        Ok(())
    }
}
```

Add a helper to select the right face for a given bold/italic combo:

```rust
fn face_for(&self, bold: bool, italic: bool) -> Option<&IDWriteFontFace> {
    match (bold, italic) {
        (true, true) => self.bold_italic_face.as_ref()
            .or(self.bold_face.as_ref())
            .or(self.font_face.as_ref()),
        (true, false) => self.bold_face.as_ref()
            .or(self.font_face.as_ref()),
        (false, true) => self.italic_face.as_ref()
            .or(self.font_face.as_ref()),
        (false, false) => self.font_face.as_ref(),
    }
}
```

Update `rasterize_glyph` to accept a font_face parameter internally — extract the rendering logic into a private `rasterize_with_face` method, and have the public `rasterize_glyph` call it with `self.font_face`.

- [ ] **Step 2: Implement the `GlyphRasterizer` trait**

Add the trait implementation at the bottom of the file (before `#[cfg(test)]`):

```rust
impl crate::glyph_rasterizer::GlyphRasterizer for DirectWriteRasterizer {
    fn load_font(&mut self, _data: &[u8], _index: u32) -> bool {
        // DirectWrite loads by family name, not bytes.
        // This is called by app.rs with embedded font bytes — we ignore it
        // because load_system_font is called separately.
        // Return true if we already have a font loaded.
        self.font_face.is_some()
    }

    fn rasterize(
        &mut self,
        ch: char,
        font_size_px: f32,
        bold: bool,
        italic: bool,
    ) -> Option<crate::glyph_rasterizer::RasterizedGlyph> {
        let face = self.face_for(bold, italic)?;

        let result = self.rasterize_with_face(face, ch, font_size_px).ok()??;

        Some(crate::glyph_rasterizer::RasterizedGlyph {
            data: result.rgb,
            format: crate::glyph_rasterizer::GlyphFormat::SubpixelRgb,
            width: result.width,
            height: result.height,
            bearing_x: result.bearing_x,
            bearing_y: result.bearing_y,
            advance: result.advance,
        })
    }

    fn measure(&mut self, font_size_px: f32) -> crate::glyph_rasterizer::MeasuredFontMetrics {
        match self.measure_font(font_size_px) {
            Some(m) => crate::glyph_rasterizer::MeasuredFontMetrics {
                ascent: m.ascent,
                descent: m.descent,
                leading: m.leading,
                average_advance: m.average_advance,
                is_monospace: true, // terminal fonts are always monospace
            },
            None => crate::glyph_rasterizer::MeasuredFontMetrics {
                ascent: font_size_px * 0.8,
                descent: font_size_px * 0.2,
                leading: 0.0,
                average_advance: font_size_px * 0.6,
                is_monospace: true,
            },
        }
    }

    fn has_glyph(&self, ch: char) -> bool {
        let Some(face) = &self.font_face else {
            return false;
        };
        unsafe {
            let codepoints = [ch as u32];
            let mut indices = [0u16; 1];
            if face.GetGlyphIndices(codepoints.as_ptr(), 1, indices.as_mut_ptr()).is_err() {
                return false;
            }
            indices[0] != 0
        }
    }
}
```

- [ ] **Step 3: Extract `rasterize_with_face` from `rasterize_glyph`**

Refactor the existing `rasterize_glyph` so its core logic is in a private method that accepts a `&IDWriteFontFace`:

```rust
fn rasterize_with_face(
    &self,
    font_face: &IDWriteFontFace,
    ch: char,
    font_size_px: f32,
) -> windows::core::Result<Option<RasterizedGlyphDW>> {
    // ... (move the existing body of rasterize_glyph here,
    //      replacing self.font_face.as_ref().ok_or(...)? with the parameter)
}

pub fn rasterize_glyph(
    &self,
    ch: char,
    font_size_px: f32,
) -> windows::core::Result<Option<RasterizedGlyphDW>> {
    let font_face = self
        .font_face
        .as_ref()
        .ok_or(windows::core::Error::from(E_FAIL))?;
    self.rasterize_with_face(font_face, ch, font_size_px)
}
```

- [ ] **Step 4: Build and run existing tests**

Run: `cargo test -p godly-terminal-surface --lib directwrite_rasterizer`

Expected: All 8 existing tests pass (create_factory, load_consolas, load_nonexistent_font_fails, rasterize_ascii_a, rasterize_without_font_fails, measure_font_metrics, measure_font_without_load_returns_none, missing_glyph_returns_none, rasterize_multiple_sizes).

- [ ] **Step 5: Add tests for the trait implementation**

Add these tests to the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn trait_rasterize_produces_subpixel_rgb() {
    use crate::glyph_rasterizer::{GlyphFormat, GlyphRasterizer as _};
    let mut rasterizer = DirectWriteRasterizer::new().unwrap();
    rasterizer.load_system_font("Consolas").unwrap();
    let glyph = rasterizer.rasterize('A', 14.0, false, false).unwrap();
    assert_eq!(glyph.format, GlyphFormat::SubpixelRgb);
    assert!(glyph.width > 0);
    assert!(glyph.height > 0);
    // SubpixelRgb: 3 bytes per pixel
    assert_eq!(glyph.data.len(), (glyph.width * glyph.height * 3) as usize);
}

#[test]
fn trait_rasterize_bold() {
    use crate::glyph_rasterizer::GlyphRasterizer as _;
    let mut rasterizer = DirectWriteRasterizer::new().unwrap();
    rasterizer.load_system_font("Consolas").unwrap();
    let normal = rasterizer.rasterize('A', 14.0, false, false).unwrap();
    let bold = rasterizer.rasterize('A', 14.0, true, false).unwrap();
    // Both should produce valid glyphs
    assert!(normal.width > 0);
    assert!(bold.width > 0);
}

#[test]
fn trait_has_glyph() {
    use crate::glyph_rasterizer::GlyphRasterizer as _;
    let mut rasterizer = DirectWriteRasterizer::new().unwrap();
    rasterizer.load_system_font("Consolas").unwrap();
    assert!(rasterizer.has_glyph('A'));
    assert!(rasterizer.has_glyph('0'));
    assert!(!rasterizer.has_glyph('\u{F0000}'));
}

#[test]
fn trait_measure() {
    use crate::glyph_rasterizer::GlyphRasterizer as _;
    let mut rasterizer = DirectWriteRasterizer::new().unwrap();
    rasterizer.load_system_font("Consolas").unwrap();
    let m = rasterizer.measure(14.0);
    assert!(m.ascent > 0.0);
    assert!(m.descent > 0.0);
    assert!(m.average_advance > 0.0);
    assert!(m.is_monospace);
}
```

- [ ] **Step 6: Run all tests**

Run: `cargo test -p godly-terminal-surface --lib directwrite_rasterizer`

Expected: All tests pass (8 existing + 4 new).

- [ ] **Step 7: Commit**

```
feat: implement GlyphRasterizer trait for DirectWriteRasterizer

Bridges the DirectWrite ClearType rasterizer to the pixel renderer's
GlyphRasterizer trait, producing SubpixelRgb glyphs with bold/italic
font face selection. refs #841
```

---

### Task 2: Wire DirectWrite into `app.rs` and enable pixel renderer

**Files:**
- Modify: `iced-shell/src/app.rs`

Replace the `SwashRasterizer` with a `Box<dyn GlyphRasterizer>` so we can conditionally use DirectWrite on Windows. Enable the pixel renderer by default.

- [ ] **Step 1: Change the rasterizer field to a trait object**

In `app.rs`, find the struct field (line ~560):

```rust
    /// Primary font rasterizer (swash-based).
    glyph_rasterizer: godly_terminal_surface::swash_rasterizer::SwashRasterizer,
```

Replace with:

```rust
    /// Primary font rasterizer.
    /// On Windows: DirectWrite (ClearType subpixel RGB).
    /// On other platforms: swash (grayscale alpha).
    glyph_rasterizer: Box<dyn godly_terminal_surface::glyph_rasterizer::GlyphRasterizer>,
```

- [ ] **Step 2: Update the Default impl to use DirectWrite on Windows**

In `app.rs`, find the initialization block (line ~717):

```rust
            glyph_rasterizer: {
                use godly_terminal_surface::glyph_rasterizer::GlyphRasterizer as _;
                let mut r = godly_terminal_surface::swash_rasterizer::SwashRasterizer::new();
                r.load_font(include_bytes!("../fonts/GeistMono-Regular.ttf"), 0);
                r
            },
```

Replace with:

```rust
            glyph_rasterizer: {
                #[cfg(windows)]
                {
                    match godly_terminal_surface::directwrite_rasterizer::DirectWriteRasterizer::new() {
                        Ok(mut dw) => {
                            if dw.load_system_font(DEFAULT_FONT_FAMILY).is_ok() {
                                log::info!("[FONT] Using DirectWrite ClearType rasterizer");
                                Box::new(dw)
                            } else {
                                log::warn!("[FONT] DirectWrite: font '{}' not found, falling back to swash", DEFAULT_FONT_FAMILY);
                                let mut r = godly_terminal_surface::swash_rasterizer::SwashRasterizer::new();
                                r.load_font(include_bytes!("../fonts/GeistMono-Regular.ttf"), 0);
                                Box::new(r)
                            }
                        }
                        Err(e) => {
                            log::warn!("[FONT] DirectWrite init failed ({e}), falling back to swash");
                            let mut r = godly_terminal_surface::swash_rasterizer::SwashRasterizer::new();
                            r.load_font(include_bytes!("../fonts/GeistMono-Regular.ttf"), 0);
                            Box::new(r)
                        }
                    }
                }
                #[cfg(not(windows))]
                {
                    let mut r = godly_terminal_surface::swash_rasterizer::SwashRasterizer::new();
                    r.load_font(include_bytes!("../fonts/GeistMono-Regular.ttf"), 0);
                    Box::new(r)
                }
            },
```

- [ ] **Step 3: Enable the pixel renderer by default**

In `app.rs`, find (line ~724):

```rust
            use_pixel_renderer: false,
```

Replace with:

```rust
            use_pixel_renderer: true,
```

- [ ] **Step 4: Update font family change handler to reload DirectWrite**

In `app.rs`, find the `Message::FontFamilyChanged` handler (line ~4270). After the existing code that updates `self.font_family` and `self.terminal_font`, add glyph cache invalidation and DirectWrite font reload:

```rust
            Message::FontFamilyChanged(name) => {
                log::info!("[FONT] family changed: {} -> {}", self.font_family, name);
                self.font_family = name.clone();
                let interned = font_enumerator::intern_font_name(&name);
                self.terminal_font = iced::Font {
                    family: iced::font::Family::Name(interned),
                    weight: iced::font::Weight::Normal,
                    stretch: iced::font::Stretch::Normal,
                    style: iced::font::Style::Normal,
                };
                // Reload the rasterizer for the new font family.
                #[cfg(windows)]
                {
                    if let Ok(mut dw) = godly_terminal_surface::directwrite_rasterizer::DirectWriteRasterizer::new() {
                        if dw.load_system_font(&name).is_ok() {
                            dw.set_scale_factor(self.window_scale_factor);
                            self.glyph_rasterizer = Box::new(dw);
                        }
                    }
                }
                self.glyph_cache.invalidate();
                return self.resize_all_terminals();
            }
```

- [ ] **Step 5: Update DPI scale factor change to propagate to DirectWrite**

In `app.rs`, find the `Message::ScaleFactorChanged` handler (line ~2521). The glyph cache is already invalidated there. We need to also update the DirectWrite rasterizer's scale factor. Since the rasterizer is now `Box<dyn GlyphRasterizer>`, we need to add a `set_scale_factor` method to the trait, OR use a simpler approach: recreate the rasterizer on DPI change (the cache is already invalidated anyway).

The simplest approach: add an optional `set_scale_factor` method to `GlyphRasterizer` with a default no-op:

In `terminal-surface/src/glyph_rasterizer.rs`, add to the trait:

```rust
    /// Update the DPI scale factor. Default is a no-op (for rasterizers that
    /// receive scale via font_size_px). DirectWrite needs this to pass the
    /// correct pixels-per-DIP to CreateGlyphRunAnalysis.
    fn set_scale_factor(&mut self, _scale: f32) {}
```

Then in `DirectWriteRasterizer`'s trait impl, override it:

```rust
    fn set_scale_factor(&mut self, scale: f32) {
        self.scale_factor = scale;
    }
```

Then in the `ScaleFactorChanged` handler in `app.rs`, after the existing `self.glyph_cache.invalidate()` line, add:

```rust
                    self.glyph_rasterizer.set_scale_factor(sf);
```

- [ ] **Step 6: Fix the `render_terminal_image` call to work with `Box<dyn>`**

In `app.rs` at line ~9663, the render call passes `&mut self.glyph_rasterizer`. Since the field is now `Box<dyn GlyphRasterizer>`, we need to dereference:

```rust
        let (pixels, w, h) = self.pixel_renderer.render(
            &grid_clone,
            &metrics,
            &mut self.glyph_cache,
            &mut *self.glyph_rasterizer,  // deref Box to get &mut dyn
            default_fg,
            default_bg,
            None,
        );
```

Actually, `&mut *self.glyph_rasterizer` and `&mut self.glyph_rasterizer` should both work since `pixel_renderer.render()` already takes `&mut dyn GlyphRasterizer`. But verify: the signature is `rasterizer: &mut dyn GlyphRasterizer`. Passing `&mut *self.glyph_rasterizer` where the field is `Box<dyn GlyphRasterizer>` will give `&mut dyn GlyphRasterizer` — correct.

- [ ] **Step 7: Build the iced-shell crate**

Run: `cargo build -p godly-terminal-iced-shell`

Expected: Compiles without errors.

- [ ] **Step 8: Commit**

```
feat: enable DirectWrite ClearType pixel renderer by default

On Windows, the terminal now uses DirectWrite for glyph rasterization,
producing ClearType 3x1 subpixel RGB bitmaps. Falls back to swash
(grayscale) on non-Windows or if DirectWrite init fails.

The pixel renderer (CPU-composited image) is now the default rendering
path instead of the iced canvas, enabling gamma-correct subpixel
blending. refs #841
```

---

### Task 3: Handle font size and DPI changes for pixel renderer re-rendering

**Files:**
- Modify: `iced-shell/src/app.rs`

When the pixel renderer is active, font size changes, DPI changes, and font family changes need to re-render all terminal images. The grid fetch already triggers `render_terminal_image`, but font/DPI changes happen without a new grid fetch.

- [ ] **Step 1: Add a helper to re-render all terminal images**

In `app.rs`, add this method to the `impl GodlyApp` block, near `render_terminal_image`:

```rust
    /// Re-render pixel-buffer images for all terminals that have grid data.
    /// Called after font metrics or glyph cache changes (font size, DPI, family).
    fn rerender_all_terminal_images(&mut self) {
        if !self.use_pixel_renderer {
            return;
        }
        let ids: Vec<String> = self.terminals.keys().cloned().collect();
        for id in ids {
            self.render_terminal_image(&id);
        }
    }
```

(Note: `self.terminals.keys()` — check the actual type. `self.terminals` is used via `.get()` and `.get_mut()` with string keys, so it's likely a HashMap or similar. Adjust if needed based on the actual type.)

- [ ] **Step 2: Call rerender after font size changes**

In the `FontSizeIncrement` handler (line ~4288), after `self.font_metrics = ...`:

```rust
                self.glyph_cache.invalidate();
                self.rerender_all_terminal_images();
                return self.resize_all_terminals();
```

In the `FontSizeDecrement` handler (line ~4301), after `self.font_metrics = ...`:

```rust
                self.glyph_cache.invalidate();
                self.rerender_all_terminal_images();
                return self.resize_all_terminals();
```

- [ ] **Step 3: Call rerender after DPI change**

In the `ScaleFactorChanged` handler (line ~2521), after `self.glyph_cache.invalidate()`:

```rust
                    self.glyph_rasterizer.set_scale_factor(sf);
                    self.rerender_all_terminal_images();
                    return self.resize_all_terminals();
```

- [ ] **Step 4: Call rerender after font family change**

In the `FontFamilyChanged` handler, after `self.glyph_cache.invalidate()`:

```rust
                self.rerender_all_terminal_images();
                return self.resize_all_terminals();
```

- [ ] **Step 5: Build and verify**

Run: `cargo build -p godly-terminal-iced-shell`

Expected: Compiles without errors.

- [ ] **Step 6: Commit**

```
fix: re-render pixel buffers on font/DPI changes

When using the pixel renderer, font size changes, DPI scale changes,
and font family changes now re-render all terminal images immediately
instead of waiting for the next grid fetch. refs #841
```

---

### Task 4: Manual visual verification

**Files:** None (testing only)

- [ ] **Step 1: Build the release binary**

Run: `cargo build --release -p godly-terminal-iced-shell`

- [ ] **Step 2: Launch and verify ClearType is active**

Launch the built binary. Check the log output for:
```
[FONT] Using DirectWrite ClearType rasterizer
```

If you see `falling back to swash` instead, the DirectWrite initialization failed — debug from there.

- [ ] **Step 3: Visual comparison**

Compare text crispness against the canvas renderer. To toggle, temporarily change `use_pixel_renderer: true` back to `false`, rebuild, and compare. The pixel renderer with DirectWrite should show:
- Sharper character edges (especially on thin strokes)
- Subtle color fringing on character edges (ClearType's RGB subpixels)
- Consistent rendering at all font sizes

- [ ] **Step 4: Test font size changes**

Use Ctrl+= and Ctrl+- to change font size. Text should re-render crisply at each size without artifacts or stale glyphs.

- [ ] **Step 5: Test with different fonts**

Change the font family in settings. The DirectWrite rasterizer should reload with the new system font. Verify text renders correctly with fonts like "Cascadia Code", "Consolas", "JetBrains Mono".
