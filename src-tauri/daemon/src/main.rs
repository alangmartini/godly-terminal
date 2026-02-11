mod debug_log;
mod pid;
mod server;
mod session;

use crate::pid::{remove_pid_file, write_pid_file, DaemonLock};
use crate::server::DaemonServer;

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

    debug_log::init();
    debug_log::install_panic_hook();
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
