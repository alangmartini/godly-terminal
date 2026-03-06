use std::sync::Arc;

use godly_app_adapter::ports::DaemonRunner;
use godly_app_adapter::{
    CallbackNotificationPort, DaemonPortConfig, LogNotificationPort, NativeDaemonPort,
};
use godly_ports::SessionSpec;
use godly_protocol::types::SessionInfo;
use godly_protocol::ShellType;
use godly_testkit::contracts::{
    daemon_lifecycle_expectation, run_daemon_lifecycle_contract, run_notification_contract,
    verify_daemon_lifecycle_outcome,
};
use godly_testkit::{FakeDaemonPort, FakeNotificationPort};
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

#[derive(Default)]
struct RecordingRunner {
    calls: Mutex<Vec<RunnerCall>>,
    sessions: Mutex<Vec<SessionInfo>>,
}

impl RecordingRunner {
    fn calls(&self) -> Vec<RunnerCall> {
        self.calls.lock().clone()
    }
}

impl DaemonRunner for RecordingRunner {
    fn create_terminal(
        &self,
        id: &str,
        shell_type: ShellType,
        cwd: Option<&str>,
        rows: u16,
        cols: u16,
    ) -> Result<(), String> {
        self.calls.lock().push(RunnerCall::Create {
            id: id.to_string(),
            shell_type: shell_type.clone(),
            cwd: cwd.map(str::to_string),
            rows,
            cols,
        });
        self.sessions.lock().push(SessionInfo {
            id: id.to_string(),
            shell_type,
            pid: 100,
            rows,
            cols,
            cwd: cwd.map(str::to_string),
            created_at: 0,
            attached: true,
            running: true,
            scrollback_rows: 0,
            scrollback_memory_bytes: 0,
            paused: false,
            title: String::new(),
        });
        Ok(())
    }

    fn close_terminal(&self, session_id: &str) -> Result<(), String> {
        self.calls.lock().push(RunnerCall::Close {
            session_id: session_id.to_string(),
        });
        self.sessions
            .lock()
            .retain(|session| session.id != session_id);
        Ok(())
    }

    fn write(&self, session_id: &str, bytes: &[u8]) -> Result<(), String> {
        self.calls.lock().push(RunnerCall::Write {
            session_id: session_id.to_string(),
            bytes: bytes.to_vec(),
        });
        Ok(())
    }

    fn resize(&self, session_id: &str, rows: u16, cols: u16) -> Result<(), String> {
        self.calls.lock().push(RunnerCall::Resize {
            session_id: session_id.to_string(),
            rows,
            cols,
        });
        if let Some(session) = self
            .sessions
            .lock()
            .iter_mut()
            .find(|session| session.id == session_id)
        {
            session.rows = rows;
            session.cols = cols;
        }
        Ok(())
    }

    fn list_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        Ok(self.sessions.lock().clone())
    }
}

#[test]
fn daemon_port_lifecycle_contract_matches_fake_port_expectations() {
    let mut fake = FakeDaemonPort::default();
    let fake_outcome = run_daemon_lifecycle_contract(
        &mut fake,
        SessionSpec {
            cwd: Some("C:\\repo".into()),
            shell: Some("pwsh".into()),
        },
    )
    .expect("fake daemon should satisfy lifecycle contract");

    let runner = Arc::new(RecordingRunner::default());
    let runner_trait: Arc<dyn DaemonRunner> = runner.clone();
    let mut native = NativeDaemonPort::from_runner_for_tests(
        runner_trait,
        DaemonPortConfig::new(ShellType::Windows, 0, 0),
        || "native-session-1".to_string(),
    );
    let native_outcome = run_daemon_lifecycle_contract(
        &mut native,
        SessionSpec {
            cwd: Some("C:\\repo".into()),
            shell: Some("pwsh".into()),
        },
    )
    .expect("native daemon adapter should satisfy lifecycle contract");

    verify_daemon_lifecycle_outcome(&fake_outcome, &daemon_lifecycle_expectation("session-1"))
        .expect("fake daemon should satisfy shared lifecycle expectation");
    verify_daemon_lifecycle_outcome(
        &native_outcome,
        &daemon_lifecycle_expectation("native-session-1"),
    )
    .expect("native adapter should satisfy shared lifecycle expectation");

    assert_eq!(
        runner.calls(),
        vec![
            RunnerCall::Create {
                id: "native-session-1".to_string(),
                shell_type: ShellType::Pwsh,
                cwd: Some("C:\\repo".to_string()),
                rows: 1,
                cols: 1,
            },
            RunnerCall::Write {
                session_id: "native-session-1".to_string(),
                bytes: b"echo contract\r".to_vec(),
            },
            RunnerCall::Resize {
                session_id: "native-session-1".to_string(),
                rows: 1,
                cols: 1,
            },
            RunnerCall::Close {
                session_id: "native-session-1".to_string(),
            },
        ]
    );
}

#[test]
fn notification_contract_is_shared_between_fake_and_adapter_ports() {
    let mut fake = FakeNotificationPort::default();
    let expected_sequence = run_notification_contract(&mut fake, "Build done", "Terminal finished")
        .expect("fake notification contract should pass");

    let recorded = Arc::new(Mutex::new(Vec::<(String, String)>::new()));
    let mut callback = CallbackNotificationPort::new({
        let recorded = Arc::clone(&recorded);
        move |title, body| {
            recorded.lock().push((title.to_string(), body.to_string()));
            Ok(())
        }
    });

    let callback_sequence =
        run_notification_contract(&mut callback, "Build done", "Terminal finished")
            .expect("callback notification contract should pass");

    let mut log_port = LogNotificationPort::new();
    let log_sequence = run_notification_contract(&mut log_port, "Build done", "Terminal finished")
        .expect("log notification contract should pass");

    assert_eq!(callback_sequence, expected_sequence);
    assert_eq!(log_sequence, expected_sequence);
    assert_eq!(fake.notifications, expected_sequence);
    assert_eq!(recorded.lock().clone(), expected_sequence);
}
