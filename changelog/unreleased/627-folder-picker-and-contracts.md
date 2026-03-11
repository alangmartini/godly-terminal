### Added
- **Workspace folder picker dialog** - clicking "New Workspace" in the sidebar now opens an OS-native folder picker dialog; selected folder becomes the workspace root and workspace name ([#627](https://github.com/alangmartini/godly-terminal/pull/627))
- **Contract testing framework** - comprehensive testing infrastructure for native Iced shell with contract definitions, fixtures, and assertions
- **New MCP query handlers** - added `workspace.details`, `terminal.cwd`, `terminal.idle` query support for better test integration

### Changed
- **Contract naming** - renamed `workspace-folder-picker` contract to `workspace-folder-path` for consistency
- **Testing documentation** - added comprehensive testing architecture guide in `testing/README.md`
