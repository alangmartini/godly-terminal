### Fixed
- **Workspace CWD not applied to new terminals** — new terminals now inherit the active workspace's `folder_path` as their working directory (#669)
- **PowerShell profile overrides workspace CWD** — PowerShell/pwsh sessions now run `Set-Location` after profile to ensure workspace CWD sticks (#669)
