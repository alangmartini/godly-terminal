### Changed
- **Tab bar visual polish** — increased height (32→36px), button height (26→30px), and accent indicator thickness (2→3px) for better visual hierarchy
- **Tab icons** — replaced 2-letter pill badges with Unicode glyphs: pwsh (❯), cmd (►), wsl (⌘), bash/zsh/fish (▸), ssh (→), ruby/irb (◈)
- **Close button styling** — circular shape with subtle danger hover effect (15% opacity), danger text on hover
- **Active tab contrast** — accent-tinted background with distinct pressed state for inactive tabs
- **UI refinements** — softer corners (4→6px border radius), tighter spacing, larger "+" button

### Tests
- Added process_icon_glyph tests for new process type icons
- Updated tab_icon tests to verify all new process types
