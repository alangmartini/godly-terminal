use godly_ports::{DaemonPort, NotificationPort, SessionSnapshot, SessionSpec};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonLifecycleOutcome {
    pub created_session_id: String,
    pub sessions_after_create: Vec<SessionSnapshot>,
    pub sessions_after_close: Vec<SessionSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonLifecycleExpectation {
    pub expected_created_session_id: String,
    pub expected_sessions_after_create_len: usize,
    pub expected_sessions_after_close_len: usize,
}

pub fn daemon_lifecycle_expectation(
    expected_created_session_id: impl Into<String>,
) -> DaemonLifecycleExpectation {
    DaemonLifecycleExpectation {
        expected_created_session_id: expected_created_session_id.into(),
        expected_sessions_after_create_len: 1,
        expected_sessions_after_close_len: 0,
    }
}

pub fn verify_daemon_lifecycle_outcome(
    outcome: &DaemonLifecycleOutcome,
    expectation: &DaemonLifecycleExpectation,
) -> Result<(), String> {
    if outcome.created_session_id != expectation.expected_created_session_id {
        return Err(format!(
            "expected created session id '{}', got '{}'",
            expectation.expected_created_session_id, outcome.created_session_id
        ));
    }

    let created_len = outcome.sessions_after_create.len();
    if created_len != expectation.expected_sessions_after_create_len {
        return Err(format!(
            "expected {} sessions after create, got {}",
            expectation.expected_sessions_after_create_len, created_len
        ));
    }

    if !outcome
        .sessions_after_create
        .iter()
        .any(|session| session.id == expectation.expected_created_session_id)
    {
        return Err(format!(
            "created session '{}' missing from sessions_after_create",
            expectation.expected_created_session_id
        ));
    }

    let close_len = outcome.sessions_after_close.len();
    if close_len != expectation.expected_sessions_after_close_len {
        return Err(format!(
            "expected {} sessions after close, got {}",
            expectation.expected_sessions_after_close_len, close_len
        ));
    }

    if outcome
        .sessions_after_close
        .iter()
        .any(|session| session.id == expectation.expected_created_session_id)
    {
        return Err(format!(
            "created session '{}' still present after close",
            expectation.expected_created_session_id
        ));
    }

    Ok(())
}

pub fn notification_contract_sequence(title: &str, body: &str) -> Vec<(String, String)> {
    vec![
        (title.to_string(), body.to_string()),
        (String::new(), String::new()),
    ]
}

/// Shared expectations for any `DaemonPort` implementation.
pub fn run_daemon_lifecycle_contract<P: DaemonPort>(
    port: &mut P,
    spec: SessionSpec,
) -> Result<DaemonLifecycleOutcome, String> {
    let created_session_id = port.create_session(spec)?;
    port.write(&created_session_id, b"echo contract\r")?;
    port.resize(&created_session_id, 0, 0)?;
    let sessions_after_create = port.list_sessions()?;
    port.close_session(&created_session_id)?;
    let sessions_after_close = port.list_sessions()?;

    Ok(DaemonLifecycleOutcome {
        created_session_id,
        sessions_after_create,
        sessions_after_close,
    })
}

/// Shared expectations for any `NotificationPort` implementation.
pub fn run_notification_contract<P: NotificationPort>(
    port: &mut P,
    title: &str,
    body: &str,
) -> Result<Vec<(String, String)>, String> {
    let sequence = notification_contract_sequence(title, body);
    for (title, body) in &sequence {
        port.notify(title, body)?;
    }
    Ok(sequence)
}
