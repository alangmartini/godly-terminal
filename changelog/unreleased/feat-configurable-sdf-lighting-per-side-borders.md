### Changed

- **Per-side border widths in SDF shader** — replaced uniform `border_width: f32` with `border_widths: vec4<f32>` (top, right, bottom, left in CSS order), enabling asymmetric borders in a single SDF quad draw call. Fragment shader uses miter diagonals to select effective border width per-pixel.
