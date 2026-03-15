### Added
- **Status bar** — New 24px bottom bar displays shell type, current working directory, and terminal dimensions (cols×rows)

### Changed
- **Title bar redesign** — Terminal icon via canvas, workspace name display, height increased 30→34px, subtle gradient background, wider button padding
- **Tab bar visual upgrade** — Tab text truncation at 30 chars, active tab uses theme BG_PRIMARY with 2px accent border, expanded process badges (Node.js→ND, Python→PY, etc.), softer badge styling, borderless add-tab button default state
- **Sidebar depth** — Active workspace left accent border, hidden 0-count badges, low-alpha accent badge background, header separator, CLAUDE.md footer with dot prefix and smaller text, bottom padding increase, subtle resize handle
- **Theme color tuning** — Dusk bg_primary warmth increased (purple undertone), success color added to ThemePalette with per-theme values, border alpha softened (0.75), shadow accessors added (SHADOW_ACCENT, SHADOW_DANGER)
- **Toast polish** — 3px left accent border, body uses TEXT_SECONDARY color, accent-tinted shadow, softer border styling
- **Scrollbar polish** — Invisible track by default, brighter thumb, rounded thumb ends
- **Empty state** — Card expanded 360→400px, text updated to "No terminals in this workspace", Ctrl+T hint added, softer border, increased padding
- **Settings dialog** — Header increased 18→20px, content padding increased 10→16px, softer outer border (0.88→0.60 alpha), footer padding increase
- **Terminal pane borders** — Unfocused border softened (1.0→0.5px with lower alpha), radius adjusted (8→6), focused pane gets accent-tinted shadow glow
- **Confirm dialog** — Cancel button rendered borderless (text-only), danger mode tints border and shadow with danger color, accent-tinted shadow for normal mode, button spacing tightened (10→8)
