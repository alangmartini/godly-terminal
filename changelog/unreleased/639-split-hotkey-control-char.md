### Fixed
- **Windows Ctrl+\ keyboard shortcut routing** — Fixed backslash key handling where Windows transmits Ctrl+\ as the control character (\x1c) instead of the literal backslash character. The shortcut resolver now correctly matches both representations (#639)
