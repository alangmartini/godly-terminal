### Changed
- **Configurable text rendering parameters** — atlas shader gamma, contrast, and coverage attenuation are now runtime-tunable GPU uniforms instead of hardcoded constants. Default coverage_attenuation=0.92 thins glyphs to match browser text weight.

### Fixed
- **Shader variable naming** — renamed misleading `fg_srgb`/`bg_srgb` to `fg_lin`/`bg_lin` in dark-background compositing path (values are linearized, not sRGB)
- **sRGB constant in transparent-bg path** — hardcoded 2.2 instead of using tunable `dark_bg_gamma` param for the fixed sRGB linearization step
