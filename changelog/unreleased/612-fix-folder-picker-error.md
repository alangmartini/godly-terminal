### Fixed
- **Folder picker error handling** — wrap native folder picker `open()` in try-catch so COM/permission errors are logged instead of silently swallowing the workspace creation flow (#612)
