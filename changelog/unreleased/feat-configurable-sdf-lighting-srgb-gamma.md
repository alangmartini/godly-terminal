### Fixed

- **Dark-background text compositing uses sRGB gamma** — The atlas shader's grayscale AA path for dark backgrounds now linearizes with sRGB gamma 2.2 instead of ClearType gamma 1.8, matching how browsers composite text on dark backgrounds for more accurate text weight and contrast.
