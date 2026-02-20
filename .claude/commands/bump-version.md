Bump the project version and create a git tag.

Accepts an optional argument: `patch` (default), `minor`, `major`, or an explicit `X.Y.Z` version.

## Steps

1. Run `npm run version:bump -- <arg>` from the repo root, where `<arg>` is the bump type or explicit version from the user's input. Default to `patch` if no argument given.
2. Verify the output shows all 8 files updated successfully.
3. Stage all changed files and commit: `chore: bump version to X.Y.Z`
4. Create an annotated git tag: `git tag -a vX.Y.Z -m "Release X.Y.Z"`
5. Report the old version, new version, and tag name.

## Files Updated

The bump script (`scripts/bump-version.mjs`) updates these files:
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src-tauri/protocol/Cargo.toml`
- `src-tauri/daemon/Cargo.toml`
- `src-tauri/mcp/Cargo.toml`
- `src-tauri/godly-vt/Cargo.toml`
- `src-tauri/notify/Cargo.toml`

Do NOT push. The user will push when ready, or `production_build.ps1` handles push automatically.
