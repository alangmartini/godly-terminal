### Added
- **Runtime-configurable text rendering parameters** — atlas shader gamma, enhanced contrast, luminance threshold, dark-bg gamma, and coverage attenuation are now exposed as a GPU uniform buffer (`TextRenderParams`) instead of hardcoded WGSL constants, enabling runtime tuning of text weight and compositing without recompilation
