### Fixed
- **Custom keybindings now resolve correctly** — Added ShortcutResolver that checks custom overrides before defaults, fixing issue where settings UI captured key combos but resolution remained hardcoded
- **Custom keybindings now persist** — Keybinding overrides are saved to `keybindings.json` and loaded on app startup

### Tests
- Added 12 new tests for ShortcutResolver (custom bindings, conflicting defaults, chord normalization, flat index mapping)
- Added persistence round-trip tests and error handling tests
