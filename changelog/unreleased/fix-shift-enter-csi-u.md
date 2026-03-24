### Fixed
- **Shift+Enter now sends CSI u escape sequence** — Enter key with modifiers (Shift, Ctrl, Alt) now sends CSI u format sequences (`\x1b[13;{mod}u`) instead of always sending `\r`. This allows tools like Claude Code that use the Kitty keyboard protocol to distinguish Shift+Enter for newline insertion.
