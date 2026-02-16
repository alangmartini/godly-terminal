# WMI Daemon stderr Panic (test_04 hang)

## Symptom
`test_04_wmi_launch_escapes_job_object` hangs indefinitely (60+ seconds) on iteration 2+.
The daemon reads a Ping request but never sends a Pong response.

## Root Cause

When the daemon is launched via WMI (`Win32_Process.Create`), it runs without a console.
The daemon calls `FreeConsole()` which invalidates stderr/stdout handles. Any subsequent
`eprintln!()` call panics with `ERROR_NO_DATA` (error 232: "The pipe is being closed").

The panic occurs in `handle_client()` at the `eprintln!("[daemon] Entering request loop for client")` line, killing the async handler before it can process any requests.

### Why it hangs instead of failing

The `io_thread` detects `resp_rx` channel disconnection (since the async handler died),
but only `break`s out of the inner response-reading loop instead of stopping the outer
polling loop. So the io_thread keeps running forever, the pipe stays open, and the test
client blocks on `read_message()` waiting for a response that will never come.

### Why iteration 1 usually works

Iteration 1 succeeds because the daemon process was freshly launched via WMI and the
`eprintln!` calls happen before `FreeConsole()` has fully invalidated the handles.
The timing is OS-dependent, which makes this flaky.

## Fix

1. **`daemon/src/main.rs`**: After `FreeConsole()`, redirect stdout/stderr to NUL using
   `SetStdHandle()`. This makes `eprintln!` silently discard output instead of panicking.

2. **`daemon/src/server.rs`**: When `resp_rx` disconnects in the io_thread, set
   `io_running = false` to stop the io_thread. This is a defense-in-depth fix so that
   even if the async handler dies for any reason, the io_thread won't hang forever.

## Verification

All 50 daemon tests pass, including 3 iterations of `test_04_wmi_launch_escapes_job_object`.
