### Added

- **Multi-stop gradient support in SDF shader** - The SDF quad pipeline now supports 3-stop symmetric gradients evaluated in the fragment shader with sRGB-correct linear-space interpolation. New `gradient_color_mid` and `gradient_config` vertex fields enable configurable gradient direction (horizontal/vertical) and mid-stop position. Progress bar now uses a single 3-stop gradient call instead of two overlapping 2-stop quads.
- **Per-side border widths in SDF shader** - Replaced uniform `border_width: f32` with `border_widths: [f32; 4]` (top, right, bottom, left) enabling CSS-style asymmetric borders. Fragment shader uses miter diagonals for correct per-side border selection. New `fill_rounded_border_sides` builder method for convenient per-side border rendering.
