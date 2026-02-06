mod pid;
mod server;
mod session;

use crate::pid::{is_daemon_running, remove_pid_file, write_pid_file};
use crate::server::DaemonServer;

#[tokio::main]
async fn main() {
    eprintln!("[daemon] Godly Terminal daemon starting (pid: {})", std::process::id());

    // Check if another instance is already running
    if is_daemon_running() {
        eprintln!("[daemon] Another daemon instance is already running, exiting");
        std::process::exit(0);
    }

    // Detach from console on Windows (so closing the launching terminal doesn't kill us)
    #[cfg(windows)]
    {
        use winapi::um::wincon::FreeConsole;
        unsafe {
            FreeConsole();
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
