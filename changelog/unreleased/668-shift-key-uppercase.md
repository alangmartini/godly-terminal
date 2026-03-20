### Fixed
- **Shift+key now produces correct characters** — Shift+letter combinations now produce uppercase letters; Shift+symbol combinations produce the correct shifted symbols on all keyboard layouts (#668)

### Tests
- Added regression tests for Shift+letter handling in `key_to_pty_bytes`
