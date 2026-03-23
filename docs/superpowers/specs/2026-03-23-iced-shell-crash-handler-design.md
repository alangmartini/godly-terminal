# Iced-Shell Crash Handler

## Problem

The iced-shell (native Godly Terminal frontend) crashes silently with zero evidence.
When the process dies — whether from a Rust panic, a wgpu/iced internal failure, or a
Windows structured exception (access violation, stack overflow) — no log entry is
written. The daemon logs just stop mid-ReadRichGrid, the bridge-exit.log records
"request_rx disconnected", and we have no idea what happened.

This has occurred at least twice. On reopening, workspaces are missing and no terminals
are open because the layout was never cleanly saved.

## Root Cause of the Logging Gap

1. `env_logger::init()` in `main.rs` writes to stdout/stderr, which go nowhere for a
   Windows GUI process (no console attached).
2. The `diag` module in `app.rs` only logs during `update()` calls — a panic in the
   rendering pipeline bypasses it entirely.
3. No `std::panic::set_hook` is installed — panics use the default handler which writes
   to stderr (which goes nowhere).
4. No Windows SEH handler — access violations and stack overflows kill the process
   instantly with no log.

## Design

### New Module: `crash_handler.rs`

Location: `src-tauri/native/iced-shell/src/crash_handler.rs`

Mirrors the daemon's `debug_log.rs` pattern (which already has `install_panic_hook()`
and `install_exception_handler()`), with improvements noted below.

### Relationship to Existing `diag` Module

The `diag` module in `app.rs` is **kept as-is**. It serves a different purpose:
operational event logging during normal execution (TerminalOutput, GridFetched,
Heartbeat, etc.). `crash_handler` is solely for crash evidence — it only writes on
startup (marker) and on fatal events (panic/SEH). The two modules are complementary:
`iced-diag.log` tells you what the app was doing, `iced-crash.log` tells you why it
died.

### Log File

- Path: `%APPDATA%/com.godly.terminal/iced-crash.log`
- Mode: append (crash history survives restarts)
- Rotation: 2MB max, rotates to `iced-crash.prev.log`
- Startup marker written on init for crash-to-session correlation
- `init()` must call `fs::create_dir_all()` for the app data directory and fall back
  to `std::env::temp_dir()` if creation fails (mirrors daemon's `debug_log::init()`)

### Initialization Order

```rust
fn main() -> iced::Result {
    crash_handler::init();                // first — before anything else
    crash_handler::install_panic_hook();
    crash_handler::install_exception_handler();

    env_logger::init();
    // ... iced application setup
}
```

Crash handlers install before iced or env_logger, so even startup crashes are captured.

### Layer 1: Panic Hook

- Installed via `std::panic::set_hook` (saves previous hook via `take_hook`)
- Captures: payload string, source location (file:line:col), full backtrace
- **Backtrace**: calls `std::backtrace::Backtrace::force_capture()` and writes the
  result directly to the crash log file. Does NOT rely on the default hook for
  backtrace output (the default hook writes to stderr, which goes nowhere for a GUI
  process).
- Uses `try_lock` on log mutex to avoid deadlock if panic occurs during logging
- Calls the previous hook afterward (for any downstream handlers)
- Flushes immediately

**Release build caveat**: The workspace release profile uses `strip = true`, which
removes symbol names from the binary. `Backtrace::force_capture()` will produce raw
addresses instead of function names. The panic payload and source location
(`PanicInfo::location()`) are always available regardless of strip settings and are
typically sufficient for crash diagnosis. Raw addresses can be resolved later with a
debug build if needed.

Output format:
```
[1774275268.927] [  128.434s] PANIC at iced-shell/src/app.rs:342:5
Payload: called `Result::unwrap()` on an `Err` value: SurfaceLost
Backtrace:
   0: 0x7ff6a2b31234
   1: 0x7ff6a2b45678
   ...
```

### Layer 2: Windows SEH Handler

- Installed via `SetUnhandledExceptionFilter`
- Does not chain to a previous filter. Iced/wgpu/winit do not install their own SEH
  handlers, so there is nothing to chain to. This matches the daemon's proven approach.
- Catches: ACCESS_VIOLATION (0xC0000005), STACK_OVERFLOW (0xC00000FD),
  HEAP_CORRUPTION (0xC0000374), STACK_BUFFER_OVERRUN (0xC0000409), others as UNKNOWN
- Uses `try_lock` — attempts direct file write as last resort if mutex is held
- cfg-gated: no-op on non-Windows

Output format:
```
[1774275268.927] FATAL EXCEPTION: code=0xC0000005 (ACCESS_VIOLATION) address=0x7FF6A2B31234
```

### Layer 3: wgpu Device Loss

The terminal uses CPU pixel rendering (PixelRenderer -> image::Handle::from_rgba()),
not direct wgpu. Iced manages wgpu internally. Device-lost events surface as panics
within iced's code, which the panic hook catches. No separate wgpu handler needed.

## Dependencies

- Add `winapi` workspace reference to `src-tauri/native/iced-shell/Cargo.toml`
  (workspace dep already defined in root Cargo.toml with `errhandlingapi` and `winnt`
  features)

## Files Changed

1. **New**: `src-tauri/native/iced-shell/src/crash_handler.rs` — the module
2. **Edit**: `src-tauri/native/iced-shell/src/main.rs` — add `mod crash_handler`,
   call init/install functions at start of `main()`
3. **Edit**: `src-tauri/native/iced-shell/Cargo.toml` — add winapi dependency

## Testing

- Verify crash handler initializes by checking `iced-crash.log` for startup marker
- Inject a deliberate panic in `update()` to verify panic hook captures backtrace
- Verify the existing E2E test suite still passes (no regression from initialization
  order change)
