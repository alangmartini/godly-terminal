# Iced-Shell Crash Handler Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add crash logging to the iced-shell so that panics, wgpu failures, and Windows structured exceptions leave evidence in `iced-crash.log` instead of vanishing silently.

**Architecture:** New `crash_handler` module in iced-shell, mirroring the daemon's `debug_log.rs` pattern. Three layers: rotating log file, Rust panic hook with backtrace, Windows SEH handler. Installed first in `main()` before any iced code runs.

**Tech Stack:** Rust std (panic hooks, backtrace), winapi (SetUnhandledExceptionFilter), existing workspace infrastructure.

**Spec:** `docs/superpowers/specs/2026-03-23-iced-shell-crash-handler-design.md`

---

## File Map

| Action | Path | Responsibility |
|--------|------|----------------|
| Create | `src-tauri/native/iced-shell/src/crash_handler.rs` | Log file init, panic hook, SEH handler |
| Modify | `src-tauri/native/iced-shell/src/main.rs` | Wire up crash handler before iced starts |
| Modify | `src-tauri/native/iced-shell/Cargo.toml` | Add winapi dependency |

---

### Task 1: Add winapi dependency to iced-shell

**Files:**
- Modify: `src-tauri/native/iced-shell/Cargo.toml`

- [ ] **Step 1: Add platform-gated winapi dep**

Add after the existing `[dependencies]` section in `Cargo.toml`:

```toml
[target.'cfg(windows)'.dependencies]
winapi.workspace = true
```

This mirrors how the daemon crate (`src-tauri/daemon/Cargo.toml:24-25`) references the workspace winapi dep. The workspace already defines winapi 0.3 with `errhandlingapi` and `winnt` features in the root `Cargo.toml:25-46`.

- [ ] **Step 2: Verify it compiles**

Run: `cargo check -p godly-iced-shell`
Expected: compiles with no errors (winapi is available but unused yet)

- [ ] **Step 3: Commit**

```bash
git add src-tauri/native/iced-shell/Cargo.toml
git commit -m "chore: add winapi dependency to iced-shell for crash handler"
```

---

### Task 2: Create crash_handler module — log file infrastructure

**Files:**
- Create: `src-tauri/native/iced-shell/src/crash_handler.rs`

Reference: `src-tauri/daemon/src/debug_log.rs` — the daemon's working implementation of the same pattern.

- [ ] **Step 1: Write the crash_handler module with init() and log()**

Create `src-tauri/native/iced-shell/src/crash_handler.rs` with the log file infrastructure. This is modeled directly on the daemon's `debug_log.rs` (lines 1-170) with these differences:
- File name: `iced-crash.log` / `iced-crash.prev.log`
- Uses `try_lock()` instead of `lock()` in `log()` — safer for crash contexts
- Writes a startup marker on init

```rust
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct LogState {
    file: std::fs::File,
    path: PathBuf,
    prev_path: PathBuf,
    bytes_written: u64,
}

static LOG_FILE: OnceLock<Mutex<LogState>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();

const MAX_LOG_SIZE: u64 = 2 * 1024 * 1024;

/// Initialize the crash log file.
/// Must be called first in main(), before any iced code.
pub fn init() {
    START_TIME.get_or_init(Instant::now);

    let app_data = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
    let dir_name = format!(
        "com.godly.terminal{}",
        godly_protocol::instance_suffix()
    );
    let dir = PathBuf::from(app_data).join(&dir_name);
    fs::create_dir_all(&dir).ok();

    let path = dir.join("iced-crash.log");
    let prev_path = dir.join("iced-crash.prev.log");

    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() > MAX_LOG_SIZE {
            let _ = fs::copy(&path, &prev_path);
            let _ = fs::remove_file(&path);
        }
    }

    let initial_size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path);

    match file {
        Ok(f) => {
            LOG_FILE.get_or_init(|| {
                Mutex::new(LogState {
                    file: f,
                    path: path.clone(),
                    prev_path: prev_path.clone(),
                    bytes_written: initial_size,
                })
            });
        }
        Err(_) => {
            let fallback_path = std::env::temp_dir().join("iced-crash.log");
            let fallback_prev = std::env::temp_dir().join("iced-crash.prev.log");
            if let Ok(f) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&fallback_path)
            {
                let initial = fs::metadata(&fallback_path).map(|m| m.len()).unwrap_or(0);
                LOG_FILE.get_or_init(|| {
                    Mutex::new(LogState {
                        file: f,
                        path: fallback_path,
                        prev_path: fallback_prev,
                        bytes_written: initial,
                    })
                });
            }
        }
    }

    log(&format!(
        "=== iced-shell crash handler initialized (v{}) ===",
        env!("GODLY_APP_VERSION")
    ));
}

/// Write a timestamped line to the crash log.
/// Uses try_lock to avoid deadlock in panic/exception contexts.
pub fn log(msg: &str) {
    if let Some(mutex) = LOG_FILE.get() {
        if let Ok(mut state) = mutex.try_lock() {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default();
            let elapsed = START_TIME
                .get()
                .map(|s| s.elapsed())
                .unwrap_or_default();
            let line = format!(
                "[{}.{:03}] [{:>8.3}s] {}\n",
                ts.as_secs(),
                ts.subsec_millis(),
                elapsed.as_secs_f64(),
                msg
            );
            let _ = state.file.write_all(line.as_bytes());
            let _ = state.file.flush();
            state.bytes_written += line.len() as u64;

            if state.bytes_written > MAX_LOG_SIZE {
                rotate(&mut state);
            }
        }
    }
}

fn rotate(state: &mut LogState) {
    let _ = fs::copy(&state.path, &state.prev_path);
    let _ = fs::remove_file(&state.path);
    if let Ok(f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&state.path)
    {
        state.file = f;
        state.bytes_written = 0;
    }
}
```

- [ ] **Step 2: Wire the module into main.rs**

In `src-tauri/native/iced-shell/src/main.rs`, add the module declaration after line 55 (`mod phone_remote;`):

```rust
mod crash_handler;
```

And modify `main()` to call `crash_handler::init()` as the very first line, before `env_logger::init()`:

```rust
fn main() -> iced::Result {
    crash_handler::init();

    env_logger::init();
    // ... rest unchanged
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p godly-iced-shell`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/native/iced-shell/src/crash_handler.rs src-tauri/native/iced-shell/src/main.rs
git commit -m "feat: add crash handler log infrastructure to iced-shell"
```

---

### Task 3: Add panic hook with backtrace capture

**Files:**
- Modify: `src-tauri/native/iced-shell/src/crash_handler.rs`
- Modify: `src-tauri/native/iced-shell/src/main.rs`

Reference: daemon's `install_panic_hook()` at `src-tauri/daemon/src/debug_log.rs:97-127`, but we add `Backtrace::force_capture()`.

- [ ] **Step 1: Add install_panic_hook() to crash_handler.rs**

Append to the end of `crash_handler.rs`:

```rust
/// Install a panic hook that writes crash info + backtrace to the log.
/// Without this, iced-shell panics vanish (no console attached to GUI process).
pub fn install_panic_hook() {
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "unknown".to_string());

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown payload".to_string()
        };

        let bt = std::backtrace::Backtrace::force_capture();

        let msg = format!(
            "PANIC at {}\nPayload: {}\nBacktrace:\n{}",
            location, payload, bt
        );

        log(&msg);

        // Also write to stderr in case a console is attached
        eprintln!("[iced-shell] {}", msg);

        // Chain to previous hook
        prev_hook(info);
    }));
}
```

- [ ] **Step 2: Wire into main.rs**

Add the call after `crash_handler::init()` in `main()`:

```rust
fn main() -> iced::Result {
    crash_handler::init();
    crash_handler::install_panic_hook();

    env_logger::init();
    // ... rest unchanged
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p godly-iced-shell`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/native/iced-shell/src/crash_handler.rs src-tauri/native/iced-shell/src/main.rs
git commit -m "feat: add panic hook with backtrace to iced-shell crash handler"
```

---

### Task 4: Add Windows SEH handler

**Files:**
- Modify: `src-tauri/native/iced-shell/src/crash_handler.rs`
- Modify: `src-tauri/native/iced-shell/src/main.rs`

Reference: daemon's `install_exception_handler()` at `src-tauri/daemon/src/debug_log.rs:172-232`, with the addition of chaining the previous filter.

- [ ] **Step 1: Add install_exception_handler() to crash_handler.rs**

Append to the end of `crash_handler.rs`:

```rust
/// Install a Windows structured exception handler for silent crashes.
/// Catches access violations, stack overflows, heap corruption that bypass
/// Rust's panic machinery entirely.
///
/// Note: does not chain to a previous filter. Iced/wgpu/winit do not install
/// their own SEH handlers, so there is nothing to chain to. This matches the
/// daemon's proven approach in debug_log.rs.
#[cfg(windows)]
pub fn install_exception_handler() {
    use winapi::um::errhandlingapi::SetUnhandledExceptionFilter;
    use winapi::um::winnt::EXCEPTION_POINTERS;

    unsafe extern "system" fn handler(info: *mut EXCEPTION_POINTERS) -> i32 {
        if info.is_null() {
            return 1; // EXCEPTION_EXECUTE_HANDLER
        }

        let record = (*info).ExceptionRecord;
        if record.is_null() {
            return 1;
        }

        let code = (*record).ExceptionCode;
        let address = (*record).ExceptionAddress as usize;

        let code_name = match code {
            0xC0000005 => "ACCESS_VIOLATION",
            0xC00000FD => "STACK_OVERFLOW",
            0xC0000374 => "HEAP_CORRUPTION",
            0xC0000409 => "STACK_BUFFER_OVERRUN",
            _ => "UNKNOWN",
        };

        // Use try_lock to avoid deadlock if exception occurs during logging
        if let Some(mutex) = LOG_FILE.get() {
            if let Ok(mut state) = mutex.try_lock() {
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default();
                let _ = writeln!(
                    state.file,
                    "[{}.{:03}] FATAL EXCEPTION: code=0x{:08X} ({}) address=0x{:X}",
                    ts.as_secs(),
                    ts.subsec_millis(),
                    code,
                    code_name,
                    address
                );
                let _ = state.file.flush();
            }
        }

        1 // EXCEPTION_EXECUTE_HANDLER
    }

    unsafe {
        SetUnhandledExceptionFilter(Some(handler));
    }
}

#[cfg(not(windows))]
pub fn install_exception_handler() {
    // No-op on non-Windows
}
```

- [ ] **Step 2: Wire into main.rs**

Add the call after `install_panic_hook()` in `main()`:

```rust
fn main() -> iced::Result {
    crash_handler::init();
    crash_handler::install_panic_hook();
    crash_handler::install_exception_handler();

    env_logger::init();
    // ... rest unchanged
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo check -p godly-iced-shell`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add src-tauri/native/iced-shell/src/crash_handler.rs src-tauri/native/iced-shell/src/main.rs
git commit -m "feat: add Windows SEH handler to iced-shell crash handler"
```

---

### Task 5: Verify end-to-end with a build

**Files:** None (verification only)

- [ ] **Step 1: Full build check**

Run: `cargo build -p godly-iced-shell`
Expected: builds successfully with no warnings about the crash_handler module

- [ ] **Step 2: Verify crash log file is created on startup**

After building, run the binary briefly. Check that `%APPDATA%/com.godly.terminal/iced-crash.log` exists and contains the startup marker line:
```
=== iced-shell crash handler initialized (v0.17.0) ===
```

- [ ] **Step 3: Verify panic hook captures backtrace**

Temporarily add a deliberate panic to verify the hook works. In `src-tauri/native/iced-shell/src/app.rs`, add at the top of the `update()` method:

```rust
// TEMPORARY: verify crash handler — remove after testing
if matches!(message, Message::Heartbeat) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| panic!("CRASH_HANDLER_TEST: deliberate panic to verify logging"));
}
```

Run the binary. It should crash on the first heartbeat. Verify `iced-crash.log` contains:
- `PANIC at` with file/line info
- `Payload: CRASH_HANDLER_TEST: deliberate panic to verify logging`
- `Backtrace:` section (addresses in release, symbols in debug)

**Remove the temporary panic code after verification.**

- [ ] **Step 4: Create changelog fragment**

Create `changelog/unreleased/<PR-number>-crash-handler.md`:

```markdown
### Added
- **Crash handler for iced-shell** — Panics, wgpu failures, and Windows structured exceptions (access violations, stack overflows) now log to `iced-crash.log` in the app data directory instead of vanishing silently. ([#<PR>])
```

- [ ] **Step 5: Final commit**

```bash
git add changelog/unreleased/
git commit -m "docs: add changelog fragment for crash handler"
```
