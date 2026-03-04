use std::process::Command;
use std::sync::Arc;
use std::time::SystemTime;

use godly_ports::{
    ClipboardPort, ClockPort, DaemonPort, NotificationPort, SessionSnapshot, SessionSpec,
};
use godly_protocol::types::SessionInfo;
use godly_protocol::ShellType;

use crate::clipboard;
use crate::commands;
use crate::daemon_client::NativeDaemonClient;

const MIN_DIMENSION: u16 = 1;
const DEFAULT_ROWS: u16 = 24;
const DEFAULT_COLS: u16 = 80;
const DEFAULT_NOTIFY_COMMAND: &str = if cfg!(windows) {
    "godly-notify.exe"
} else {
    "godly-notify"
};
#[derive(Debug, Clone, PartialEq)]
pub struct DaemonPortConfig {
    pub default_shell: ShellType,
    pub default_rows: u16,
    pub default_cols: u16,
}

impl Default for DaemonPortConfig {
    fn default() -> Self {
        Self {
            default_shell: ShellType::Windows,
            default_rows: DEFAULT_ROWS,
            default_cols: DEFAULT_COLS,
        }
    }
}

impl DaemonPortConfig {
    pub fn new(default_shell: ShellType, default_rows: u16, default_cols: u16) -> Self {
        Self {
            default_shell,
            default_rows: normalize_dimension(default_rows),
            default_cols: normalize_dimension(default_cols),
        }
    }
}

#[doc(hidden)]
pub trait DaemonRunner: Send + Sync {
    fn create_terminal(
        &self,
        id: &str,
        shell_type: ShellType,
        cwd: Option<&str>,
        rows: u16,
        cols: u16,
    ) -> Result<(), String>;

    fn close_terminal(&self, session_id: &str) -> Result<(), String>;
    fn write(&self, session_id: &str, bytes: &[u8]) -> Result<(), String>;
    fn resize(&self, session_id: &str, rows: u16, cols: u16) -> Result<(), String>;
    fn list_sessions(&self) -> Result<Vec<SessionInfo>, String>;
}

#[derive(Clone)]
struct NativeDaemonRunner {
    client: Arc<NativeDaemonClient>,
}

impl NativeDaemonRunner {
    fn new(client: Arc<NativeDaemonClient>) -> Self {
        Self { client }
    }
}

impl DaemonRunner for NativeDaemonRunner {
    fn create_terminal(
        &self,
        id: &str,
        shell_type: ShellType,
        cwd: Option<&str>,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        commands::create_terminal(&self.client, id, shell_type, cwd, rows, cols)
    }

    fn close_terminal(&self, session_id: &str) -> Result<(), String> {
        commands::close_terminal(&self.client, session_id)
    }

    fn write(&self, session_id: &str, bytes: &[u8]) -> Result<(), String> {
        commands::write_to_terminal(&self.client, session_id, bytes)
    }

    fn resize(&self, session_id: &str, rows: u16, cols: u16) -> Result<(), String> {
        commands::resize_terminal(&self.client, session_id, rows, cols)
    }

    fn list_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        commands::list_sessions(&self.client)
    }
}

pub struct NativeDaemonPort {
    runner: Arc<dyn DaemonRunner>,
    config: DaemonPortConfig,
    session_id_factory: Arc<dyn Fn() -> String + Send + Sync>,
}

impl NativeDaemonPort {
    pub fn new(client: Arc<NativeDaemonClient>) -> Self {
        Self::with_config(client, DaemonPortConfig::default())
    }

    pub fn from_client(client: NativeDaemonClient) -> Self {
        Self::new(Arc::new(client))
    }

    pub fn with_config(client: Arc<NativeDaemonClient>, config: DaemonPortConfig) -> Self {
        Self::with_parts(
            Arc::new(NativeDaemonRunner::new(client)),
            config,
            Arc::new(|| uuid::Uuid::new_v4().to_string()),
        )
    }

    #[doc(hidden)]
    pub fn from_runner_for_tests<F>(
        runner: Arc<dyn DaemonRunner>,
        config: DaemonPortConfig,
        session_id_factory: F,
    ) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        Self::with_parts(runner, config, Arc::new(session_id_factory))
    }

    fn with_parts(
        runner: Arc<dyn DaemonRunner>,
        mut config: DaemonPortConfig,
        session_id_factory: Arc<dyn Fn() -> String + Send + Sync>,
    ) -> Self {
        config.default_rows = normalize_dimension(config.default_rows);
        config.default_cols = normalize_dimension(config.default_cols);
        Self {
            runner,
            config,
            session_id_factory,
        }
    }

    pub fn with_session_id_factory<F>(mut self, session_id_factory: F) -> Self
    where
        F: Fn() -> String + Send + Sync + 'static,
    {
        self.session_id_factory = Arc::new(session_id_factory);
        self
    }

    pub fn set_default_size(&mut self, rows: u16, cols: u16) {
        self.config.default_rows = normalize_dimension(rows);
        self.config.default_cols = normalize_dimension(cols);
    }

    pub fn default_size(&self) -> (u16, u16) {
        (self.config.default_rows, self.config.default_cols)
    }

    pub fn set_default_shell(&mut self, shell_type: ShellType) {
        self.config.default_shell = shell_type;
    }

    pub fn default_shell(&self) -> &ShellType {
        &self.config.default_shell
    }
}

impl DaemonPort for NativeDaemonPort {
    fn create_session(&mut self, spec: SessionSpec) -> Result<String, String> {
        let session_id = (self.session_id_factory)();
        let shell_type = resolve_shell_type(spec.shell.as_deref(), &self.config.default_shell);
        self.runner.create_terminal(
            &session_id,
            shell_type,
            spec.cwd.as_deref(),
            self.config.default_rows,
            self.config.default_cols,
        )?;
        Ok(session_id)
    }

    fn close_session(&mut self, session_id: &str) -> Result<(), String> {
        self.runner.close_terminal(session_id)
    }

    fn write(&mut self, session_id: &str, bytes: &[u8]) -> Result<(), String> {
        self.runner.write(session_id, bytes)
    }

    fn resize(&mut self, session_id: &str, rows: u16, cols: u16) -> Result<(), String> {
        self.runner.resize(
            session_id,
            normalize_dimension(rows),
            normalize_dimension(cols),
        )
    }

    fn list_sessions(&self) -> Result<Vec<SessionSnapshot>, String> {
        self.runner
            .list_sessions()
            .map(|sessions| sessions.into_iter().map(session_info_to_snapshot).collect())
    }
}

/// Convenience bundle used by shell wiring to access all concrete ports.
pub struct NativePortBundle {
    pub daemon: NativeDaemonPort,
    pub clipboard: SystemClipboardPort,
    pub notifications: SystemNotificationPort,
    pub clock: SystemClockPort,
}

impl NativePortBundle {
    pub fn connect() -> Result<Self, String> {
        let client = NativeDaemonClient::connect_or_launch()?;
        Ok(Self::from_client(client))
    }

    pub fn from_client(client: NativeDaemonClient) -> Self {
        Self::from_shared_client(Arc::new(client))
    }

    pub fn from_shared_client(client: Arc<NativeDaemonClient>) -> Self {
        Self {
            daemon: NativeDaemonPort::new(client),
            clipboard: SystemClipboardPort::new(),
            notifications: SystemNotificationPort::new(),
            clock: SystemClockPort::new(),
        }
    }
}

/// Connect to daemon and create a concrete `DaemonPort` adapter.
pub fn connect_daemon_port() -> Result<NativeDaemonPort, String> {
    Ok(NativeDaemonPort::from_client(
        NativeDaemonClient::connect_or_launch()?,
    ))
}

/// Create a concrete clipboard adapter.
pub fn system_clipboard_port() -> SystemClipboardPort {
    SystemClipboardPort::new()
}

/// Create a concrete notification adapter.
pub fn log_notification_port() -> LogNotificationPort {
    LogNotificationPort::new()
}

/// Create a concrete system notification adapter.
pub fn system_notification_port() -> SystemNotificationPort {
    SystemNotificationPort::new()
}

/// Create a concrete clock adapter.
pub fn system_clock_port() -> SystemClockPort {
    SystemClockPort::new()
}

fn normalize_dimension(value: u16) -> u16 {
    value.max(MIN_DIMENSION)
}

fn resolve_shell_type(spec_shell: Option<&str>, default_shell: &ShellType) -> ShellType {
    let shell = match spec_shell.map(str::trim) {
        Some(value) if !value.is_empty() => value,
        _ => return default_shell.clone(),
    };

    if shell.eq_ignore_ascii_case("windows") || shell.eq_ignore_ascii_case("powershell") {
        return ShellType::Windows;
    }
    if shell.eq_ignore_ascii_case("pwsh") || shell.eq_ignore_ascii_case("pwsh.exe") {
        return ShellType::Pwsh;
    }
    if shell.eq_ignore_ascii_case("cmd") || shell.eq_ignore_ascii_case("cmd.exe") {
        return ShellType::Cmd;
    }
    if shell.eq_ignore_ascii_case("wsl") || shell.eq_ignore_ascii_case("wsl.exe") {
        return ShellType::Wsl { distribution: None };
    }

    if let Some((prefix, distribution)) = shell.split_once(':') {
        if prefix.eq_ignore_ascii_case("wsl") {
            let distro = distribution.trim();
            return ShellType::Wsl {
                distribution: (!distro.is_empty()).then(|| distro.to_owned()),
            };
        }
    }

    let mut parts = shell.split_whitespace();
    let program = parts.next().unwrap_or(shell).to_string();
    let args: Vec<String> = parts.map(str::to_owned).collect();
    ShellType::Custom {
        program,
        args: (!args.is_empty()).then_some(args),
    }
}

fn session_info_to_snapshot(session: SessionInfo) -> SessionSnapshot {
    SessionSnapshot {
        id: session.id,
        title: session.title,
        process_name: session.shell_type.display_name(),
        exited: !session.running,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClipboardPort;

impl SystemClipboardPort {
    pub fn new() -> Self {
        Self
    }
}

impl ClipboardPort for SystemClipboardPort {
    fn read_text(&self) -> Result<String, String> {
        clipboard::paste_from_clipboard()
    }
}

#[derive(Debug, Clone)]
pub struct SystemNotificationPort {
    command: String,
    terminal_id: Option<String>,
}

impl SystemNotificationPort {
    pub fn new() -> Self {
        Self {
            command: DEFAULT_NOTIFY_COMMAND.to_string(),
            terminal_id: None,
        }
    }

    pub fn with_command(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            terminal_id: None,
        }
    }

    pub fn set_terminal_id(&mut self, terminal_id: Option<String>) {
        self.terminal_id = terminal_id;
    }

    pub fn command(&self) -> &str {
        &self.command
    }
}

impl Default for SystemNotificationPort {
    fn default() -> Self {
        Self::new()
    }
}

impl NotificationPort for SystemNotificationPort {
    fn notify(&mut self, title: &str, body: &str) -> Result<(), String> {
        let mut command = Command::new(&self.command);
        if let Some(terminal_id) = &self.terminal_id {
            command.args(["--terminal-id", terminal_id]);
        }

        let message = notification_message(title, body);
        if !message.is_empty() {
            command.arg(message);
        }

        let output = command.output().map_err(|e| {
            format!(
                "Failed to run notification command '{}': {}",
                self.command, e
            )
        })?;

        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let details = if !stderr.is_empty() {
            stderr
        } else if !stdout.is_empty() {
            stdout
        } else {
            format!("exit status {}", output.status)
        };

        Err(format!(
            "Notification command '{}' failed: {}",
            self.command, details
        ))
    }
}

pub struct CallbackNotificationPort {
    notify_fn: Box<dyn FnMut(&str, &str) -> Result<(), String> + Send>,
}

impl CallbackNotificationPort {
    pub fn new<F>(notify_fn: F) -> Self
    where
        F: FnMut(&str, &str) -> Result<(), String> + Send + 'static,
    {
        Self {
            notify_fn: Box::new(notify_fn),
        }
    }
}

impl Default for CallbackNotificationPort {
    fn default() -> Self {
        Self::new(|_, _| Ok(()))
    }
}

impl NotificationPort for CallbackNotificationPort {
    fn notify(&mut self, title: &str, body: &str) -> Result<(), String> {
        (self.notify_fn)(title, body)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LogNotificationPort;

impl LogNotificationPort {
    pub fn new() -> Self {
        Self
    }
}

impl NotificationPort for LogNotificationPort {
    fn notify(&mut self, title: &str, body: &str) -> Result<(), String> {
        log::info!("notification [{}] {}", title, body);
        Ok(())
    }
}

fn notification_message(title: &str, body: &str) -> String {
    let clean_title = title.trim();
    let clean_body = body.trim();

    match (clean_title.is_empty(), clean_body.is_empty()) {
        (true, true) => String::new(),
        (false, true) => clean_title.to_string(),
        (true, false) => clean_body.to_string(),
        (false, false) => format!("{}: {}", clean_title, clean_body),
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClockPort;

impl SystemClockPort {
    pub fn new() -> Self {
        Self
    }
}

impl ClockPort for SystemClockPort {
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::VecDeque;
    use std::time::Duration;

    use parking_lot::Mutex;

    #[derive(Debug, Clone, PartialEq)]
    enum RunnerCall {
        Create {
            id: String,
            shell_type: ShellType,
            cwd: Option<String>,
            rows: u16,
            cols: u16,
        },
        Close {
            session_id: String,
        },
        Write {
            session_id: String,
            bytes: Vec<u8>,
        },
        Resize {
            session_id: String,
            rows: u16,
            cols: u16,
        },
    }

    struct FakeRunner {
        calls: Mutex<Vec<RunnerCall>>,
        create_results: Mutex<VecDeque<Result<(), String>>>,
        close_results: Mutex<VecDeque<Result<(), String>>>,
        write_results: Mutex<VecDeque<Result<(), String>>>,
        resize_results: Mutex<VecDeque<Result<(), String>>>,
        list_result: Mutex<Result<Vec<SessionInfo>, String>>,
    }

    impl Default for FakeRunner {
        fn default() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                create_results: Mutex::new(VecDeque::new()),
                close_results: Mutex::new(VecDeque::new()),
                write_results: Mutex::new(VecDeque::new()),
                resize_results: Mutex::new(VecDeque::new()),
                list_result: Mutex::new(Ok(Vec::new())),
            }
        }
    }

    impl FakeRunner {
        fn calls(&self) -> Vec<RunnerCall> {
            self.calls.lock().clone()
        }

        fn enqueue_create_result(&self, result: Result<(), String>) {
            self.create_results.lock().push_back(result);
        }

        fn set_list_result(&self, result: Result<Vec<SessionInfo>, String>) {
            *self.list_result.lock() = result;
        }
    }

    impl DaemonRunner for FakeRunner {
        fn create_terminal(
            &self,
            id: &str,
            shell_type: ShellType,
            cwd: Option<&str>,
            rows: u16,
            cols: u16,
        ) -> Result<(), String> {
            self.calls.lock().push(RunnerCall::Create {
                id: id.to_owned(),
                shell_type,
                cwd: cwd.map(str::to_owned),
                rows,
                cols,
            });
            self.create_results
                .lock()
                .pop_front()
                .unwrap_or_else(|| Ok(()))
        }

        fn close_terminal(&self, session_id: &str) -> Result<(), String> {
            self.calls.lock().push(RunnerCall::Close {
                session_id: session_id.to_owned(),
            });
            self.close_results
                .lock()
                .pop_front()
                .unwrap_or_else(|| Ok(()))
        }

        fn write(&self, session_id: &str, bytes: &[u8]) -> Result<(), String> {
            self.calls.lock().push(RunnerCall::Write {
                session_id: session_id.to_owned(),
                bytes: bytes.to_vec(),
            });
            self.write_results
                .lock()
                .pop_front()
                .unwrap_or_else(|| Ok(()))
        }

        fn resize(&self, session_id: &str, rows: u16, cols: u16) -> Result<(), String> {
            self.calls.lock().push(RunnerCall::Resize {
                session_id: session_id.to_owned(),
                rows,
                cols,
            });
            self.resize_results
                .lock()
                .pop_front()
                .unwrap_or_else(|| Ok(()))
        }

        fn list_sessions(&self) -> Result<Vec<SessionInfo>, String> {
            self.list_result.lock().clone()
        }
    }

    fn build_test_port(
        runner: Arc<FakeRunner>,
        config: DaemonPortConfig,
        generated_id: &'static str,
    ) -> NativeDaemonPort {
        NativeDaemonPort::with_parts(runner, config, Arc::new(move || generated_id.to_string()))
    }

    fn sample_session(shell_type: ShellType, running: bool) -> SessionInfo {
        SessionInfo {
            id: "sess-1".to_string(),
            shell_type,
            pid: 100,
            rows: 24,
            cols: 80,
            cwd: Some("C:\\work".to_string()),
            created_at: 0,
            attached: true,
            running,
            scrollback_rows: 0,
            scrollback_memory_bytes: 0,
            paused: false,
            title: "demo".to_string(),
        }
    }

    #[test]
    fn daemon_create_session_uses_defaults_and_generated_id() {
        let runner = Arc::new(FakeRunner::default());
        let mut port = build_test_port(
            Arc::clone(&runner),
            DaemonPortConfig::new(ShellType::Pwsh, 30, 120),
            "generated-1",
        );

        let session_id = port
            .create_session(SessionSpec {
                cwd: None,
                shell: None,
            })
            .expect("create should succeed");

        assert_eq!(session_id, "generated-1");
        assert_eq!(
            runner.calls(),
            vec![RunnerCall::Create {
                id: "generated-1".to_string(),
                shell_type: ShellType::Pwsh,
                cwd: None,
                rows: 30,
                cols: 120,
            }]
        );
    }

    #[test]
    fn daemon_create_session_maps_shell_aliases() {
        let runner = Arc::new(FakeRunner::default());
        let mut port = build_test_port(
            Arc::clone(&runner),
            DaemonPortConfig::new(ShellType::Windows, 24, 80),
            "generated-2",
        );

        port.create_session(SessionSpec {
            cwd: Some("C:\\repo".to_string()),
            shell: Some("wsl:Ubuntu-24.04".to_string()),
        })
        .expect("create should succeed");

        assert_eq!(
            runner.calls(),
            vec![RunnerCall::Create {
                id: "generated-2".to_string(),
                shell_type: ShellType::Wsl {
                    distribution: Some("Ubuntu-24.04".to_string()),
                },
                cwd: Some("C:\\repo".to_string()),
                rows: 24,
                cols: 80,
            }]
        );
    }

    #[test]
    fn daemon_forwards_close_write_resize() {
        let runner = Arc::new(FakeRunner::default());
        let mut port = build_test_port(
            Arc::clone(&runner),
            DaemonPortConfig::new(ShellType::Windows, 24, 80),
            "unused",
        );

        port.close_session("sess-a").expect("close should succeed");
        port.write("sess-a", b"echo hi\r")
            .expect("write should succeed");
        port.resize("sess-a", 0, 0).expect("resize should succeed");

        assert_eq!(
            runner.calls(),
            vec![
                RunnerCall::Close {
                    session_id: "sess-a".to_string(),
                },
                RunnerCall::Write {
                    session_id: "sess-a".to_string(),
                    bytes: b"echo hi\r".to_vec(),
                },
                RunnerCall::Resize {
                    session_id: "sess-a".to_string(),
                    rows: 1,
                    cols: 1,
                },
            ]
        );
    }

    #[test]
    fn daemon_list_sessions_maps_to_snapshots() {
        let runner = Arc::new(FakeRunner::default());
        runner.set_list_result(Ok(vec![sample_session(ShellType::Cmd, false)]));
        let port = build_test_port(
            Arc::clone(&runner),
            DaemonPortConfig::new(ShellType::Windows, 24, 80),
            "unused",
        );

        let sessions = port.list_sessions().expect("list should succeed");

        assert_eq!(
            sessions,
            vec![SessionSnapshot {
                id: "sess-1".to_string(),
                title: "demo".to_string(),
                process_name: "cmd".to_string(),
                exited: true,
            }]
        );
    }

    #[test]
    fn daemon_propagates_create_errors() {
        let runner = Arc::new(FakeRunner::default());
        runner.enqueue_create_result(Err("daemon unavailable".to_string()));
        let mut port = build_test_port(
            Arc::clone(&runner),
            DaemonPortConfig::new(ShellType::Windows, 24, 80),
            "generated-3",
        );

        let err = port
            .create_session(SessionSpec {
                cwd: None,
                shell: Some("pwsh".to_string()),
            })
            .expect_err("create should fail");

        assert_eq!(err, "daemon unavailable");
    }

    #[test]
    fn callback_notification_port_invokes_callback() {
        let recorded = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
        let mut port = CallbackNotificationPort::new({
            let recorded = Arc::clone(&recorded);
            move |title, body| {
                recorded.lock().push((title.to_string(), body.to_string()));
                Ok(())
            }
        });

        port.notify("Build done", "Session finished")
            .expect("notify should succeed");

        assert_eq!(
            recorded.lock().clone(),
            vec![("Build done".to_string(), "Session finished".to_string())]
        );
    }

    #[test]
    fn notification_message_combines_title_and_body() {
        assert_eq!(
            notification_message("Build done", "Session finished"),
            "Build done: Session finished"
        );
        assert_eq!(notification_message("Build done", ""), "Build done");
        assert_eq!(
            notification_message("", "Session finished"),
            "Session finished"
        );
        assert_eq!(notification_message("", ""), "");
    }

    #[test]
    fn system_notification_port_default_command_is_set() {
        let port = SystemNotificationPort::new();
        assert!(!port.command().is_empty());
    }

    #[test]
    fn system_clock_reports_recent_time() {
        let before = SystemTime::now();
        let now = SystemClockPort::new().now();
        let after = SystemTime::now();

        let lower_bound = before.checked_sub(Duration::from_secs(1)).unwrap_or(before);
        let upper_bound = after + Duration::from_secs(1);

        assert!(now >= lower_bound, "clock is unexpectedly far in the past");
        assert!(
            now <= upper_bound,
            "clock is unexpectedly far in the future"
        );
    }
}
