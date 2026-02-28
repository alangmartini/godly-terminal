# Bump Version

Bump the project version across all crates and config files, collect changelog fragments, then build to verify.

## Usage

```
/bump-version <patch|minor|major|X.Y.Z>
```

## Steps

1. Run `npm run version:bump -- <arg>` where `<arg>` is the bump type or explicit version from the user's input.
2. Verify the output shows all files updated successfully.
3. **Collect changelog fragments** into `CHANGELOG.md`:
   a. Read all `.md` files from `changelog/unreleased/` (skip `.gitkeep` and `TEMPLATE.md`).
   b. If fragments exist:
      - Group entries by section (`### Added`, `### Fixed`, `### Changed`, `### Removed`, `### Tests`).
      - Merge duplicate sections (e.g., two fragments both with `### Fixed` get combined).
      - Replace the `## [Unreleased]` line and any content below it (up to the next `## [`) with:
        ```
        ## [Unreleased]

        ## [X.Y.Z] - YYYY-MM-DD

        ### Added
        - ...

        ### Fixed
        - ...
        ```
      - Delete the fragment files (keep `.gitkeep`).
   c. If no fragments exist, warn and add `## [X.Y.Z] - YYYY-MM-DD` with empty content.
4. Run `npm run build` to confirm the frontend builds with the new version constant.
5. Run `cd src-tauri && cargo check --workspace` to confirm all Rust crates compile.
6. Commit directly to the current branch with message `chore: bump version to X.Y.Z`.
7. Create an annotated git tag: `git tag -a vX.Y.Z -m "Release X.Y.Z"`
8. Report old version, new version, tag name, and changelog entries collected.
9. Ask the user: **"Do you want to build a release installer?"**
   - If **yes**: push the commit and tag (`git push && git push origin vX.Y.Z`). This triggers the `build-installer` workflow which builds the NSIS/MSI installer and creates a draft GitHub Release.
   - If **no**: do NOT push. The user will push when ready.

## Changelog Fragment Format

Fragments live in `changelog/unreleased/` as individual `.md` files. Each contains one or more Keep a Changelog sections:

```markdown
### Fixed
- **Bug title** — description (#PR)
```

See `changelog/TEMPLATE.md` for the full format reference.

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
