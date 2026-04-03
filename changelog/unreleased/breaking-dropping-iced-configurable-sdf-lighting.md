### Changed

- **Configurable SDF surface lighting** — Added per-element `lighting_intensity` control to the SDF quad pipeline. Default changed from full 3D lighting (specular, rim, AO, grain) to flat CSS-like rendering (0.0), closing the biggest stylistic divergence from the web reference. Individual elements can opt into 3D polish via `set_lighting(1.0)`.
