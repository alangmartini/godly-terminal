### Fixed
- **tmux shim pane & I/O commands restored** — reimplemented `split-window`, `select-pane`, `list-panes`, `kill-pane`, `send-keys`, `display-message`, and `capture-pane` commands that were lost during merge conflict resolution of PRs #815 and #817 (#831)
