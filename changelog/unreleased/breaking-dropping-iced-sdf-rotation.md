### Changed

- **SDF rotation support in quad shader** — Added per-quad rotation field to the SDF fragment shader, enabling smooth anti-aliased angled shapes.
- **Close button (X) icon now uses rotated SDF pills** — Replaced ~30 overlapping SDF circles per icon with 2 rotated rounded rectangles, yielding crisper anti-aliased diagonal lines at ~10x fewer vertices.
- **Scrollbar track and dynamic opacity** — Added subtle track background and increased thumb opacity when scrolled away from live position.
