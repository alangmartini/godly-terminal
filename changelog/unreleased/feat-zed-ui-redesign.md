### Added
- **Zed One Dark theme** — new default theme with semantic UI tokens (ghost_hover, ghost_active, ghost_selected, surface_bg, tab_active_bg, tab_inactive_bg, title_bar_bg, status_bar_bg, border_variant, border_focused, etc.)
- **Semantic color tokens** — 16 new tokens in ThemePalette for consistent UI styling across all components with auto-derivation for existing themes

### Changed
- **Component restyling** — tab bar, title bar, sidebar, status bar, context menu, settings dialog, and scrollbar now use semantic theme tokens instead of hardcoded colors
- **Tab styling** — uses TAB_ACTIVE_BG/TAB_INACTIVE_BG for cleaner visual hierarchy
- **Title bar** — switched from gradient to flat TITLE_BAR_BG for modern appearance
- **Sidebar items** — workspace items now use GHOST_HOVER/GHOST_SELECTED states
- **Status bar** — uses STATUS_BAR_BG without border for seamless integration
- **Context menu** — updated to SURFACE_BG with softer shadow styling
- **Settings dialog** — tabs now use ghost-style appearance with reduced border radius
