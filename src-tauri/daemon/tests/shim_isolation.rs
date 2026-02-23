//! Test: Daemon with isolated instance must NOT discover or kill shims from other instances.
//!
//! Bug (#303): shim_metadata_dir() returned the same directory regardless of GODLY_INSTANCE,
//! so test daemons would discover production shim metadata on startup, fail to reconnect,
//! and kill the production shim processes via TerminateProcess().
//!
//! Incident: running daemon/tests/single_instance.rs killed all live terminal sessions
//! because the concurrent test daemons shared the production metadata directory.
//!
//! Run with:
//!   cd src-tauri && cargo nextest run -p godly-daemon --test shim_isolation -- --test-threads=1

#![cfg(windows)]

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::FromRawHandle;
use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use godly_protocol::frame;
use godly_protocol::{DaemonMessage, Request, Response};

use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
use winapi::um::handleapi::INVALID_HANDLE_VALUE;
use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unique_pipe_name(suffix: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!(
        r"\\.\pipe\godly-test-{}-{}-{}",
        suffix,
        std::process::id(),
        nonce
    )
}

fn daemon_binary_path() -> std::path::PathBuf {
    let exe = std::env::current_exe().unwrap();
    let deps_dir = exe.parent().unwrap();
    let debug_dir = deps_dir.parent().unwrap();
    let path = debug_dir.join("godly-daemon.exe");
    assert!(
        path.exists(),
        "Daemon binary not found at {:?}. Run `cargo build -p godly-daemon` first.",
        path
    );
    path
}

struct JobGuard {
    handle: *mut winapi::ctypes::c_void,
}

unsafe impl Send for JobGuard {}

impl JobGuard {
    fn new() -> Self {
        let handle = unsafe {
            use winapi::um::jobapi2::{CreateJobObjectW, SetInformationJobObject};
            use winapi::um::winnt::{
                JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            };

            let job = CreateJobObjectW(std::ptr::null_mut(), std::ptr::null());
            assert!(!job.is_null(), "CreateJobObjectW failed");

            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ret = SetInformationJobObject(
                job,
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut _,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            );
            assert!(ret != 0, "SetInformationJobObject failed");
            job
        };
        Self { handle }
    }

    fn assign(&self, child: &Child) {
        unsafe {
            use std::os::windows::io::AsRawHandle;
            use winapi::um::jobapi2::AssignProcessToJobObject;
            let ret = AssignProcessToJobObject(self.handle, child.as_raw_handle() as *mut _);
            assert!(ret != 0, "AssignProcessToJobObject failed");
        }
    }
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        unsafe {
            winapi::um::handleapi::CloseHandle(self.handle);
        }
    }
}

/// Spawn a daemon with both GODLY_PIPE_NAME and GODLY_INSTANCE for full isolation.
fn spawn_isolated_daemon(pipe_name: &str, instance: &str, job: &JobGuard) -> Child {
    let daemon_path = daemon_binary_path();
    let child = Command::new(&daemon_path)
        .env("GODLY_PIPE_NAME", pipe_name)
        .env("GODLY_INSTANCE", instance)
        .env("GODLY_NO_DETACH", "1")
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .expect("Failed to spawn daemon");
    job.assign(&child);
    child
}

fn try_connect_pipe(pipe_name: &str) -> Option<std::fs::File> {
    let wide_name: Vec<u16> = OsStr::new(pipe_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            wide_name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        None
    } else {
        Some(unsafe { std::fs::File::from_raw_handle(handle as _) })
    }
}

fn wait_for_pipe(pipe_name: &str, timeout: Duration) -> std::fs::File {
    let start = std::time::Instant::now();
    loop {
        if let Some(mut file) = try_connect_pipe(pipe_name) {
            if let Ok(Response::Pong) = std::panic::catch_unwind(
                std::panic::AssertUnwindSafe(|| send_request(&mut file, &Request::Ping)),
            ) {
                return file;
            }
            drop(file);
        }
        if start.elapsed() > timeout {
            panic!(
                "Pipe '{}' did not become available within {:?}",
                pipe_name, timeout
            );
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn send_request(pipe: &mut std::fs::File, req: &Request) -> Response {
    frame::write_request(pipe, req).expect("Failed to write request");
    loop {
        let msg: DaemonMessage = frame::read_daemon_message(pipe)
            .expect("Failed to read message")
            .expect("Unexpected EOF on pipe");
        match msg {
            DaemonMessage::Response(r) => return r,
            DaemonMessage::Event(_) => continue,
        }
    }
}

/// Check if a process is still alive using Windows API.
fn is_process_alive(pid: u32) -> bool {
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetExitCodeProcess, OpenProcess};
    use winapi::um::winnt::PROCESS_QUERY_LIMITED_INFORMATION;

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return false;
        }
        let mut exit_code: u32 = 0;
        let result = GetExitCodeProcess(handle, &mut exit_code as *mut u32 as *mut _);
        CloseHandle(handle);
        result != 0 && exit_code == 259 // STILL_ACTIVE
    }
}

/// The default (production) shim metadata directory, hardcoded to match
/// the buggy behavior where GODLY_INSTANCE is not consulted.
fn production_metadata_dir() -> std::path::PathBuf {
    let base = std::env::var("APPDATA").unwrap();
    std::path::PathBuf::from(base)
        .join("com.godly.terminal")
        .join("shims")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Bug #303: A test daemon discovers production shim metadata and kills foreign shims.
///
/// When shim_metadata_dir() ignores GODLY_INSTANCE, all daemons (production and test)
/// share the same metadata directory. A test daemon starting up discovers production
/// shim metadata, fails to reconnect (wrong pipes), and kills the shim processes.
///
/// This test reproduces the exact scenario:
/// 1. Write fake "production" shim metadata with a live process PID
/// 2. Start a daemon with a different GODLY_INSTANCE
/// 3. Verify the foreign process is NOT killed
#[test]
#[ntest::timeout(60_000)]
fn test_isolated_daemon_does_not_kill_foreign_shims() {
    // 1. Spawn a victim process (simulates a production pty-shim)
    let mut victim = Command::new("powershell")
        .args(["-c", "Start-Sleep 120"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("Failed to spawn victim process");
    let victim_pid = victim.id();

    // 2. Write fake shim metadata to the DEFAULT (production) metadata directory.
    //    This simulates what happens when the production daemon's shims are running.
    let session_id = format!("foreign-shim-test-{}", std::process::id());
    let fake_pipe = format!(r"\\.\pipe\godly-shim-{}", session_id);
    let metadata = godly_protocol::ShimMetadata {
        session_id: session_id.clone(),
        shim_pid: victim_pid,
        shim_pipe_name: fake_pipe,
        shell_pid: victim_pid + 1,
        shell_type: godly_protocol::types::ShellType::Windows,
        cwd: None,
        rows: 24,
        cols: 80,
        created_at: 0,
    };

    let prod_dir = production_metadata_dir();
    std::fs::create_dir_all(&prod_dir).unwrap();
    let meta_path = prod_dir.join(format!("{}.json", session_id));
    std::fs::write(
        &meta_path,
        serde_json::to_string_pretty(&metadata).unwrap(),
    )
    .unwrap();

    // 3. Start a test daemon with a DIFFERENT instance — it should NOT see the
    //    production metadata directory at all.
    let pipe_name = unique_pipe_name("shim-isolation");
    let instance = format!("test-shim-isolation-{}", std::process::id());
    let job = JobGuard::new();
    let mut daemon = spawn_isolated_daemon(&pipe_name, &instance, &job);

    // Wait for the daemon to fully start and process surviving shims
    let pipe = wait_for_pipe(&pipe_name, Duration::from_secs(10));
    drop(pipe);
    thread::sleep(Duration::from_secs(2));

    // 4. The victim process should still be alive — the test daemon should not
    //    have touched it because it should be reading from an isolated metadata dir.
    let victim_alive = is_process_alive(victim_pid);

    // 5. Cleanup
    let _ = daemon.kill();
    let _ = daemon.wait();
    let _ = victim.kill();
    let _ = victim.wait();
    let _ = std::fs::remove_file(&meta_path);

    // Clean up the instance-specific metadata dir if it was created
    let instance_dir = {
        let base = std::env::var("APPDATA").unwrap();
        std::path::PathBuf::from(base).join(format!("com.godly.terminal-{}", instance))
    };
    let _ = std::fs::remove_dir_all(&instance_dir);

    assert!(
        victim_alive,
        "ISOLATION BUG: Test daemon (instance='{}') killed a foreign shim process (pid={}). \
         shim_metadata_dir() is not scoped by GODLY_INSTANCE, so the test daemon \
         discovered production shim metadata, failed to reconnect, and killed the process.",
        instance, victim_pid
    );
}

/// Verify that shim_metadata_dir() returns different paths for different instances.
///
/// This is a unit-level check: when GODLY_INSTANCE is set, the metadata directory
/// must be scoped to that instance so different daemons don't share metadata.
///
/// NOTE: We test this in a subprocess to avoid polluting the test process's env.
#[test]
#[ntest::timeout(30_000)]
fn test_metadata_dir_scoped_by_instance() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let check_metadata_dir = |env_vars: &[(&str, &str)]| -> String {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let result_file = std::env::temp_dir().join(format!(
            "godly-shim-isolation-test-{}-{}.txt",
            std::process::id(),
            id,
        ));

        let exe = std::env::current_exe().unwrap();
        let mut cmd = Command::new(&exe);
        cmd.arg("--ignored"); // run as ignored test
        cmd.arg("__shim_metadata_dir_subprocess");

        // Clean env to isolate
        cmd.env_remove("GODLY_INSTANCE");
        cmd.env_remove("GODLY_PIPE_NAME");

        for (key, val) in env_vars {
            cmd.env(key, val);
        }
        cmd.env("GODLY_TEST_RESULT_FILE", &result_file);

        let status = cmd.status().expect("subprocess failed");
        assert!(status.success(), "subprocess exited with {:?}", status);

        let result = std::fs::read_to_string(&result_file).expect("result file missing");
        let _ = std::fs::remove_file(&result_file);
        result
    };

    let default_dir = check_metadata_dir(&[]);
    let instance_dir = check_metadata_dir(&[("GODLY_INSTANCE", "test-abc")]);

    assert_ne!(
        default_dir, instance_dir,
        "shim_metadata_dir() must return different paths for different GODLY_INSTANCE values. \
         Default: '{}', Instance='test-abc': '{}'",
        default_dir, instance_dir,
    );

    assert!(
        instance_dir.contains("test-abc"),
        "Instance-scoped dir should contain the instance name. Got: '{}'",
        instance_dir,
    );
}

/// Subprocess helper for test_metadata_dir_scoped_by_instance.
/// Writes shim_metadata_dir() result to a file so the parent can read it.
#[test]
#[ignore]
#[ntest::timeout(10_000)]
fn __shim_metadata_dir_subprocess() {
    if let Ok(path) = std::env::var("GODLY_TEST_RESULT_FILE") {
        let dir = godly_protocol::shim_metadata_dir();
        std::fs::write(path, dir.to_string_lossy().as_ref()).unwrap();
    }
}
