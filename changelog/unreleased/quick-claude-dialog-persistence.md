### Added
- **Quick Claude preferences persistence** — Dialog now remembers selected model, mode, AI tool, and workspace across sessions
- **Dynamic model discovery** — Models are automatically discovered from `claude --help` output on dialog open, with fallback to hardcoded list

### Fixed
- **Second launch broken** — Dialog launch was blocked after a failed launch attempt due to `quick_claude_launch` not being cleared on error

### Changed
- **Default Quick Claude mode is now "auto"** — Changed from "default" mode to "auto" for improved user experience
