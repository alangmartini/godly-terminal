### Fixed
- **tmux shim: parse `session:window.pane` target format** — agent team spawning failed with "target not found: undefined" because `list-panes` and `resolve_target` did exact-match on session name, rejecting the `session:window` format Claude Code passes (#847)

### Added
- **tmux shim: accept `-L <socket>` flag** — Claude Code's swarm mode passes `-L claude-swarm-<pid>` which the shim now strips instead of rejecting as an unknown command (#847)
