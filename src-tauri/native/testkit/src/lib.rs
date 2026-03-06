use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

use godly_ports::{
    ClipboardPort, ClockPort, DaemonPort, NotificationPort, SessionSnapshot, SessionSpec,
};

pub mod contracts;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DaemonCall {
    CreateSession(SessionSpec),
    CloseSession(String),
    Write {
        session_id: String,
        bytes: Vec<u8>,
    },
    Resize {
        session_id: String,
        rows: u16,
        cols: u16,
    },
    ListSessions,
}

/// In-memory fake daemon for deterministic feature tests.
#[derive(Debug, Default)]
pub struct FakeDaemonPort {
    pub calls: Vec<DaemonCall>,
    pub sessions: Vec<SessionSnapshot>,
    pub next_session_id: u32,
    pub fail_next: Option<String>,
}

impl DaemonPort for FakeDaemonPort {
    fn create_session(&mut self, spec: SessionSpec) -> Result<String, String> {
        self.calls.push(DaemonCall::CreateSession(spec));
        if let Some(err) = self.fail_next.take() {
            return Err(err);
        }
        self.next_session_id += 1;
        let id = format!("session-{}", self.next_session_id);
        self.sessions.push(SessionSnapshot {
            id: id.clone(),
            title: String::new(),
            process_name: String::new(),
            exited: false,
        });
        Ok(id)
    }

    fn close_session(&mut self, session_id: &str) -> Result<(), String> {
        self.calls
            .push(DaemonCall::CloseSession(session_id.to_owned()));
        if let Some(err) = self.fail_next.take() {
            return Err(err);
        }
        self.sessions.retain(|session| session.id != session_id);
        Ok(())
    }

    fn write(&mut self, session_id: &str, bytes: &[u8]) -> Result<(), String> {
        self.calls.push(DaemonCall::Write {
            session_id: session_id.to_owned(),
            bytes: bytes.to_vec(),
        });
        if let Some(err) = self.fail_next.take() {
            return Err(err);
        }
        Ok(())
    }

    fn resize(&mut self, session_id: &str, rows: u16, cols: u16) -> Result<(), String> {
        self.calls.push(DaemonCall::Resize {
            session_id: session_id.to_owned(),
            rows,
            cols,
        });
        if let Some(err) = self.fail_next.take() {
            return Err(err);
        }
        Ok(())
    }

    fn list_sessions(&self) -> Result<Vec<SessionSnapshot>, String> {
        Ok(self.sessions.clone())
    }
}

/// Queue-based fake clipboard.
#[derive(Debug, Default)]
pub struct FakeClipboardPort {
    pub queue: VecDeque<Result<String, String>>,
}

impl ClipboardPort for FakeClipboardPort {
    fn read_text(&self) -> Result<String, String> {
        self.queue
            .front()
            .cloned()
            .unwrap_or_else(|| Ok(String::new()))
    }
}

/// Recording fake notification sink.
#[derive(Debug, Default)]
pub struct FakeNotificationPort {
    pub notifications: Vec<(String, String)>,
}

impl NotificationPort for FakeNotificationPort {
    fn notify(&mut self, title: &str, body: &str) -> Result<(), String> {
        self.notifications.push((title.to_owned(), body.to_owned()));
        Ok(())
    }
}

/// Controllable fake clock.
#[derive(Debug, Clone)]
pub struct FakeClock {
    now: SystemTime,
}

impl FakeClock {
    pub fn new(now: SystemTime) -> Self {
        Self { now }
    }

    pub fn advance(&mut self, duration: Duration) {
        self.now += duration;
    }
}

impl ClockPort for FakeClock {
    fn now(&self) -> SystemTime {
        self.now
    }
}

#[cfg(test)]
mod tests {
    use crate::contracts::{
        daemon_lifecycle_expectation, run_daemon_lifecycle_contract, run_notification_contract,
        verify_daemon_lifecycle_outcome,
    };

    use super::*;
    use std::time::UNIX_EPOCH;

    #[test]
    fn fake_daemon_records_calls_and_creates_session() {
        let mut daemon = FakeDaemonPort::default();
        let session_id = daemon
            .create_session(SessionSpec {
                cwd: Some("C:\\".into()),
                shell: Some("pwsh".into()),
            })
            .expect("session should be created");

        assert_eq!(session_id, "session-1");
        assert_eq!(daemon.sessions.len(), 1);
        assert!(matches!(daemon.calls[0], DaemonCall::CreateSession(_)));
    }

    #[test]
    fn fake_notification_records_messages() {
        let mut notifications = FakeNotificationPort::default();
        notifications
            .notify("Terminal Done", "Build finished")
            .expect("notify should succeed");
        assert_eq!(
            notifications.notifications,
            vec![("Terminal Done".into(), "Build finished".into())]
        );
    }

    #[test]
    fn fake_clock_can_advance() {
        let mut clock = FakeClock::new(UNIX_EPOCH);
        assert_eq!(clock.now(), UNIX_EPOCH);
        clock.advance(Duration::from_secs(5));
        assert_eq!(clock.now(), UNIX_EPOCH + Duration::from_secs(5));
    }

    #[test]
    fn fake_daemon_satisfies_shared_lifecycle_contract() {
        let mut daemon = FakeDaemonPort::default();
        let outcome = run_daemon_lifecycle_contract(
            &mut daemon,
            SessionSpec {
                cwd: Some("C:\\repo".into()),
                shell: Some("pwsh".into()),
            },
        )
        .expect("daemon lifecycle contract should pass");

        verify_daemon_lifecycle_outcome(&outcome, &daemon_lifecycle_expectation("session-1"))
            .expect("lifecycle outcome should satisfy shared expectation");
        assert_eq!(daemon.calls.len(), 4);
    }

    #[test]
    fn fake_notifications_satisfy_shared_contract() {
        let mut notifications = FakeNotificationPort::default();
        let expected_sequence =
            run_notification_contract(&mut notifications, "Build done", "Terminal finished")
                .expect("notification contract should pass");

        assert_eq!(notifications.notifications, expected_sequence);
    }
}
