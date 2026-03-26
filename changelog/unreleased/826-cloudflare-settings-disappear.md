### Fixed
- **Cloudflare settings disappear on click** — wrap settings dialog in two-layer `mouse_area` widgets to prevent mouse events from leaking through the `stack!` overlay to main content, and add `scrollable` to Remote tab so Cloudflare Tunnel fields don't clip (#826)
