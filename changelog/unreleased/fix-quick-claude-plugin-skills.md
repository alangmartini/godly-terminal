### Fixed
- **Quick Claude plugin skills in autocomplete** — `/` autocomplete now discovers and displays all installed Claude Code plugins (e.g., `/commit`, `/brainstorming`, `/frontend-design`) by reading `~/.claude/plugins/installed_plugins.json` and scanning each plugin's `skills/` and `commands/` directories
