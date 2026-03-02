### Fixed
- **Staging installer no longer kills production Godly Terminal** — set `mainBinaryName` to `godly-terminal-staging` so the NSIS installer only targets the staging binary during upgrades (#504)
- **Staging app fully isolated from production at runtime** — bake `GODLY_INSTANCE=staging` into the binary via `#[cfg(feature = "staging")]` so installed staging builds use separate daemon pipes, PID files, and shim metadata without needing env vars (#504)
