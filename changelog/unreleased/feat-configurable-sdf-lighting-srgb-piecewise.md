### Changed

- **Spec-correct sRGB transfer function** — Replaced approximate `pow(x, 2.2)` sRGB conversions in the SDF quad shader with the IEC 61966-2-1 piecewise transfer function, improving color accuracy in ultra-dark UI tones where the approximation diverges most.
