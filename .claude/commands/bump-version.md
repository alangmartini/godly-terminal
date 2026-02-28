Bump the project version and create a git tag.

Accepts an optional argument: `patch` (default), `minor`, `major`, or an explicit `X.Y.Z` version.

## Steps

1. Run `npm run version:bump -- <arg>` from the repo root, where `<arg>` is the bump type or explicit version from the user's input. Default to `patch` if no argument given.
2. Verify the output shows all 8 files updated successfully.
3. Stage all changed files and commit: `chore: bump version to X.Y.Z`
4. Create an annotated git tag: `git tag -a vX.Y.Z -m "Release X.Y.Z"`
5. Report the old version, new version, and tag name.
6. Ask the user: **"Do you want to build a release installer?"**
   - If **yes**: push the commit and tag (`git push && git push origin vX.Y.Z`). This triggers the `build-installer` workflow which builds the NSIS/MSI installer and creates a draft GitHub Release.
   - If **no**: do NOT push. The user will push when ready.
