### Fixed
- **Split layouts lost on restart** — split pane configurations are now preserved when restarting the app. Previously, all splits reverted to single-pane view because layout trees were not synced to the backend or read back on restore (#498)
