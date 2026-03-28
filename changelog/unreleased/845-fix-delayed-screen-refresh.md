### Fixed

- **Terminal blinking eliminated** — Replaced `Image` widget with custom `Shader` widget for pixel-rendered terminal content. The root cause was `Handle::from_rgba()` creating a new Iced image handle ID on every render, forcing GPU texture allocation/swap/deallocation per frame. The Shader widget creates one persistent `wgpu::Texture` and updates pixels in-place via `queue.write_texture()` — zero handle churn, zero texture swap, zero blink. (#845)
