# Building the Native Frontend

The native frontend (`godly-native`) is an Iced + wgpu application that will
eventually replace the Tauri + TypeScript frontend. During the migration, both
frontends coexist.

## Quick Start

```bash
# Debug build
pnpm build:native

# Release build
pnpm build:native:release

# Run the native binary directly
./src-tauri/target/debug/godly-native
```

## Prerequisites

- Rust toolchain (same version as the main app)
- A GPU driver with Vulkan, DX12, or Metal support (wgpu requirement)
- The daemon must be running (`pnpm build:daemon` first, then start a terminal)

## Build Scripts

| Command | Description |
|---------|-------------|
| `pnpm build:native` | Debug build via `scripts/build-native.ps1` |
| `pnpm build:native:release` | Release build with optimizations |

## Crate Structure

```
src-tauri/native/
  app-adapter/         ← Tauri-free daemon client (godly-app-adapter)
  iced-shell/          ← Iced application shell (godly-iced-shell → godly-native binary)
  terminal-surface/    ← Custom Iced widget for terminal rendering (godly-terminal-surface)
  parity-harness/      ← Test infrastructure for web-vs-native comparison (godly-parity-harness)
```

All crates are workspace members defined in `src-tauri/Cargo.toml`.

## Current Status (Phase 0)

- `godly-native` opens a window with a placeholder text widget
- `app-adapter` has pipe connection logic extracted from the Tauri client
- `terminal-surface` draws a solid dark rectangle
- `parity-harness` has contract tests verifying protocol serialization
