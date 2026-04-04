### Fixed
- **SDF lighting call sites match runtime behavior** — all `fill_rounded*`, `stroke_rounded*`, `fill_shadow*`, and gradient builder methods now pass `lighting_intensity = 0.0` at the call site, matching the `UiBuilder` default that was already overriding them at emit time
