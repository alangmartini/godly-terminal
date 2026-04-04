### Added

- **Per-corner radii + per-side border API** — `fill_rounded_custom_border_sides` combines per-corner radii `[TL, TR, BR, BL]` with per-side border widths `[top, right, bottom, left]` in a single SDF quad

### Changed

- **User-message left border follows rounded corners** — Migrated from separate flat rect overlay to single SDF bordered quad with `borderRadius: "0 4px 4px 0"` and `borderLeft: 3px`, matching CSS rendering
