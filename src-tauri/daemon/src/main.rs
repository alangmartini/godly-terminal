mod debug_log;
mod pid;
mod server;
mod session;

use crate::pid::{is_daemon_running, remove_pid_file, write_pid_file};
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
    debug_log::daemon_log!("=== Daemon starting === pid={}", std::process::id());
    eprintln!("[daemon] Godly Terminal daemon starting (pid: {})", std::process::id());

    // Check if another instance is already running
    if is_daemon_running() {
        eprintln!("[daemon] Another daemon instance is already running, exiting");
        std::process::exit(0);
    }

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
        remove_pid_file();
        eprintln!("[daemon] Daemon exiting");
    };

    // Run the server
    let server = DaemonServer::new();
    server.run().await;

    cleanup();
}
