# Build Pipeline Optimization

**Status**: Resolved
**Date**: 2026-02-18

## Problem

Development iteration speed was bottlenecked by:
1. Sequential binary builds in `npm run dev` (daemon, mcp, notify built one-by-one)
2. TypeScript check (`tsc`) running on every `npm run build` (~30s)
3. `tokio = "full"` compiling all tokio features when only a subset is used
4. Redundant `winapi` feature sets across 4 crates causing unnecessary recompilation
5. No dev profile optimization (default debug settings)
6. No file watcher for Rust code changes

## Compilation Profile (Pre-Fix)

| Component | Cold Build | Warm Build |
|-----------|-----------|------------|
| godly-terminal (Tauri) | 127s | ~10s |
| godly-daemon | 15s | ~1s |
| godly-mcp | 10s | ~1s |
| godly-notify | 5s | ~0.5s |
| godly-vt | 1.6s | ~0.4s |
| tsc | 30s | 30s |

Top compile-time crates: tauri-utils (34s), tauri (23s), webview2-com-sys (22s), tao (18s), tokio (9.2s), winapi (9.1s).

## Changes Made

### Tier 1: Quick Wins

1. **Parallel binary builds** (`package.json`): Changed `npm run dev` from 3 sequential `npm run build:*` calls to a single `cargo build -p godly-daemon -p godly-mcp -p godly-notify` invocation. Cargo handles internal parallelism.

2. **Removed tsc from npm run build** (`package.json`): `build` now runs `vite build` only. Added `typecheck` for standalone checks and `build:check` for CI/production that includes tsc.

3. **Pinned tokio features**:
   - Main app: `["rt"]` (only uses `spawn_blocking`)
   - Daemon: `["rt-multi-thread", "sync", "time", "macros"]` (uses main, spawn, mpsc, sleep)

4. **Unified winapi via workspace dependencies** (`Cargo.toml`): All crates now use `winapi.workspace = true` with a shared superset of features. Eliminates redundant feature-set compilations.

### Tier 2: Medium Effort

5. **Dev profile optimization** (`Cargo.toml`):
   - `opt-level = 0`, `debug = 1` (line tables only), `incremental = true` for own code
   - `opt-level = 1` for dependencies (faster runtime without slow rebuilds)

6. **Workspace dependency inheritance**: Centralized `serde`, `serde_json`, `uuid`, `parking_lot`, `godly-protocol` in `[workspace.dependencies]`. Reduces version drift and simplifies Cargo.toml files.

7. **cargo-watch integration** (`package.json`): Added `dev:watch` script that auto-rebuilds daemon/mcp/notify on Rust source changes. Requires `cargo install cargo-watch`.

8. **Production build uses build:check** (`tauri.conf.json`): `beforeBuildCommand` now runs `build:check` (with tsc) instead of `build`.

## Expected Impact

| Scenario | Before | After |
|----------|--------|-------|
| `npm run dev` cold start | ~60s (3 sequential builds) | ~20s (1 parallel build) |
| `npm run build` (frontend) | ~31s (tsc 30s + vite 1s) | ~1s (vite only) |
| Daemon incremental rebuild | ~1s | ~1s (same, but auto-triggered by watcher) |

## Regression Risk

- **tokio feature pinning**: If new code uses tokio features not in the pinned set, compile will fail with a clear error. Fix: add the needed feature.
- **tsc removal from build**: Type errors won't be caught by `npm run build`. Mitigated by `build:check` in tauri.conf.json for production, and editor IDE checks during dev.
