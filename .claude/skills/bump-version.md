# Bump Version

Bump the project version across all crates and config files, then build to verify.

## Usage

```
/bump-version <patch|minor|major|X.Y.Z>
```

## Steps

1. Run `npm run version:bump -- <arg>` where `<arg>` is the bump type or explicit version from the user's input.
2. Verify the output shows all files updated successfully.
3. Run `npm run build` to confirm the frontend builds with the new version constant.
4. Run `cd src-tauri && cargo check --workspace` to confirm all Rust crates compile.
5. Commit directly to the current branch with message `chore: bump version to X.Y.Z` and push.

## Files Updated

The bump script updates these files:
- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`
- `src-tauri/protocol/Cargo.toml`
- `src-tauri/daemon/Cargo.toml`
- `src-tauri/mcp/Cargo.toml`
- `src-tauri/godly-vt/Cargo.toml`
- `src-tauri/notify/Cargo.toml`

## Notes

- This is a `chore:` change — commit and push directly to the current branch, no PR needed.
- The version is injected into the frontend at build time via Vite's `define` (`__APP_VERSION__`) and displayed in the Settings dialog.
