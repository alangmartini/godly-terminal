### Changed
- **Voice default model upgraded to Large v3 Turbo** — Switched from Base (142 MB) to Large v3 Turbo (1.5 GB) for significantly better transcription accuracy with technical terms like "Quick Claude", "Shift+V", keyboard shortcuts, and developer vocabulary. (refs #363)

### Added
- **Voice vocabulary hints** — Added initial_prompt with domain-specific terms (Godly Terminal, Quick Claude, Claude Code, keyboard shortcuts, etc.) to bias Whisper toward correct transcription of technical vocabulary.
- **Custom vocabulary editor** — New textarea in Voice plugin settings to add project-specific terms (comma-separated) that improve recognition without reloading the model. (refs #363)
