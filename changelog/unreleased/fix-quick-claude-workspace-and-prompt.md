### Fixed

- **Quick Claude workspace selection ignored** — User-selected workspace was ignored; terminal always opened in new workspace. Now properly adds terminal to selected workspace as a new tab.
- **Quick Claude prompt never delivered** — User prompt was never sent to Claude due to missing CWD configuration and broken trust prompt detection. Fixed by: (1) propagating workspace `folder_path` as CWD through launch chain, (2) replacing rigid 3-step trust/prompt sequence with adaptive `WaitForClaudeReady` that handles trust prompt if present.
- **Terminal working directory wrong** — Quick Claude always opened in daemon process directory. Now inherits CWD from selected workspace `folder_path`.
- **New workspace got wrong folder_path** — Defaulted to `std::env::current_dir()`. Fixed by using `add_with_details()` with workspace folder path.
- **Silent launch failures** — Errors during Quick Claude launch were silently swallowed. Now show toast notification on failure.
- **New workspace name hardcoded** — Always "Quick Claude". Now derives from prompt snippet for better UX.

### Tests

- All 41 quick_claude tests passing
- cargo check passes
