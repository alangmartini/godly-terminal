### Changed
- **Visual parity with web reference** — Godly-shell native UI now matches the web mockup's GitHub Dark palette, layout dimensions, and flat chrome style
- **Color palette** — Swapped from One Dark Pro to ultra-dark GitHub Dark (#0b0d12 base) with indigo/green/amber/violet accent rotation
- **Tab bar** — Flat active tab style with 2px bottom accent underline, transparent semi-transparent badge circles, simplified inactive state
- **Status bar** — Replaced pill-based layout with flat text: streaming indicator, path, git branch (amber), diff stats (green/red)
- **Sidebar** — Flat 3px left accent bar for active session, reduced glow/shadow effects
- **Shadows** — Removed vignettes, corner fills, breathing glows; kept subtle directional edge shadows

### Added
- **Right panel** — New contextual detail panel (380px default, header + content + status bar, close button)
- **Resize handles** — Interactive 3px drag zones between sidebar/terminal and terminal/right panel with cursor change
- **Progress bar** — 2px animated gradient bar (indigo-to-violet) between terminal and status bar during streaming
- **Layout constants** — Tab bar 36px, status bar 26px, sidebar 200px, breadcrumb removed
