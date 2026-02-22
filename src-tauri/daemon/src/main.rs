mod debug_log;
mod pid;
mod server;
mod session;
mod shim_client;
mod shim_metadata;

use crate::pid::{remove_pid_file, write_pid_file, DaemonLock};
use crate::server::DaemonServer;

/// Redirect stdout and stderr to NUL so that print macros (eprintln!, println!)
/// silently discard output instead of panicking on invalid handles.
/// Called after FreeConsole() when the daemon runs without a console.
#[cfg(windows)]
fn redirect_std_to_nul() {
    use std::fs::OpenOptions;
    use std::os::windows::io::AsRawHandle;
    use winapi::um::processenv::SetStdHandle;
    use winapi::um::winbase::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};

    if let Ok(nul) = OpenOptions::new().write(true).open("NUL") {
        let handle = nul.as_raw_handle();
        unsafe {
            SetStdHandle(STD_OUTPUT_HANDLE, handle as _);
            SetStdHandle(STD_ERROR_HANDLE, handle as _);
        }
        // Leak the handle so it stays valid for the process lifetime
        std::mem::forget(nul);
    }
}

#[cfg(feature = "leak-check")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main]
async fn main() {
    #[cfg(feature = "leak-check")]
    let _profiler = dhat::Profiler::new_heap();

    // Parse --instance arg (must happen before any protocol calls).
    // WMI-launched processes don't inherit env vars, so CLI args are the
    // only reliable way to pass the instance name through that path.
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--instance") {
        if let Some(name) = args.get(pos + 1) {
            unsafe { std::env::set_var("GODLY_INSTANCE", name) };
        }
    }

    // Set Windows timer resolution to 1ms. Without this, thread::sleep(1ms)
    // actually sleeps ~15ms due to the default 15.625ms timer resolution.
    // The daemon I/O thread uses adaptive polling with sleep(1ms) fallback;
    // this makes the fallback actually ~1ms instead of ~15ms.
    #[cfg(windows)]
    unsafe {
        winapi::um::timeapi::timeBeginPeriod(1);
    }

    debug_log::init();
    debug_log::install_panic_hook();
    debug_log::install_exception_handler();
    debug_log::daemon_log!(
        "\n========================================\n\
         === Daemon starting === pid={} args={:?}\n\
         ========================================",
        std::process::id(),
        std::env::args().collect::<Vec<_>>()
    );
    eprintln!("[daemon] Godly Terminal daemon starting (pid: {})", std::process::id());

    // Acquire singleton lock via named mutex. This is race-free — unlike the
    // previous pipe-based check, a named mutex is atomically created by the
    // kernel so two daemons cannot both succeed.
    let _lock = match DaemonLock::try_acquire() {
        Ok(lock) => lock,
        Err(msg) => {
            eprintln!("[daemon] {}, exiting", msg);
            debug_log::daemon_log!("Singleton lock failed: {}", msg);
            std::process::exit(0);
        }
    };

    // Detach from console on Windows (so closing the launching terminal doesn't kill us)
    // Skip when GODLY_NO_DETACH is set (used by tests to keep daemon as child process)
    #[cfg(windows)]
    {
        if std::env::var("GODLY_NO_DETACH").is_err() {
            use winapi::um::wincon::FreeConsole;
            unsafe {
                FreeConsole();
            }
            // After FreeConsole, stderr/stdout handles become invalid. When the
            // daemon is launched via WMI (Win32_Process.Create), any eprintln!
            // call would panic with ERROR_NO_DATA (232), killing the async
            // handler and causing the daemon to hang without responding to clients.
            // Redirect both to NUL so print macros silently discard output.
            redirect_std_to_nul();
        }
    }

    // Write PID file
    write_pid_file();

    // Set up cleanup on exit
    let cleanup = || {
        debug_log::daemon_log!("=== Daemon exiting normally === pid={}", std::process::id());
        remove_pid_file();
        eprintln!("[daemon] Daemon exiting");
    };

    // Run the server
    let server = DaemonServer::new();
    server.run().await;

    cleanup();
    // _lock is dropped here, releasing the named mutex
}
