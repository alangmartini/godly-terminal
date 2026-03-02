### Fixed
- **AI tool selection persistence** — Quick Claude dialog now remembers the user's AI tool choice (claude/codex/both) between invocations via localStorage (#495)

### Added
- **AI Tools settings page** — New settings tab for configuring custom AI tool binaries (name, binary path, launch command template, branch suffix) (#495)
- **Configurable branch suffixes** — Branch name suffixes for parallel AI tool launches are now configurable in Settings > AI Tools instead of hardcoded `-cc`/`-c` (#495)
- **Up to 4 simultaneous launches** — Quick Claude can now launch up to 4 AI tools in parallel with a 2x2 grid layout, extending the previous limit of 2 (#495)
