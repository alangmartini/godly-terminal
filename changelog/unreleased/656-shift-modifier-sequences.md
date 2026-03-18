### Fixed
- **Shift/Ctrl/Alt modifiers for special keys** — special keys (arrows, Home/End, Delete, Insert, Page Up/Down, F-keys) now properly encode xterm-style CSI modifier parameters (e.g. `\x1b[1;2D` for Shift+Left) instead of sending bare sequences. This enables terminal programs to distinguish modified from unmodified keys, restoring Shift+Arrow selection, Ctrl+Arrow word movement, and other modifier combinations (#656)

### Tests
- Added comprehensive test coverage for modifier sequences on arrow keys, Home/End, Delete/Insert, Page Up/Down, and function keys
