### Added
- **Shortcut capture mode** — Settings shortcuts tab now allows clicking on keybinding badges to enter capture mode and rebind shortcuts by pressing a key (native shell implementation)
- **MCP settings actions** — Added `settings.open`/`settings.close` commands and `settings.shortcuts.badge` query for contract-based testing of shortcut rebind workflow
- **Badge info helper** — New `get_badge_info()` function in shortcuts tab to support MCP queries on badge state and index

### Fixed
- **Contract runner assertion** — Fixed `executeAction` to properly detect action failures by checking `ok: false` in response (previously only checked `isError` flag)
- **Assertion types** — Added `not_null` assertion type as alias for `exists` for consistency in contract tests
