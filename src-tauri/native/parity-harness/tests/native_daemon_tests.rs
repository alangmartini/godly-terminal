//! Integration tests for the native daemon client.
//!
//! These tests spawn an isolated daemon per test (unique pipe name + instance)
//! and verify the core protocol operations work correctly.
//!
//! Prerequisites: `cargo build -p godly-daemon` must have been run first.

#[cfg(windows)]
mod tests {
    use godly_parity_harness::daemon_fixture::DaemonFixture;
    use godly_protocol::{Request, Response, ShellType};

    #[test]
    fn connect_and_ping() {
        let mut fixture = DaemonFixture::start("ping").expect("Failed to start daemon fixture");

        let response = fixture.send_request(&Request::Ping).expect("Ping failed");
        assert!(
            matches!(response, Response::Pong),
            "Expected Pong, got: {:?}",
            response
        );
    }

    #[test]
    fn create_session() {
        let mut fixture =
            DaemonFixture::start("create_session").expect("Failed to start daemon fixture");

        let response = fixture
            .send_request(&Request::CreateSession {
                id: "test-session-1".into(),
                shell_type: ShellType::Windows,
                cwd: None,
                rows: 24,
                cols: 80,
                env: None,
            })
            .expect("CreateSession failed");

        match response {
            Response::SessionCreated { session } => {
                assert_eq!(session.id, "test-session-1");
            }
            other => panic!("Expected SessionCreated, got: {:?}", other),
        }
    }

    #[test]
    fn write_and_read_grid() {
        let mut fixture =
            DaemonFixture::start("write_read_grid").expect("Failed to start daemon fixture");

        // Create session
        let response = fixture
            .send_request(&Request::CreateSession {
                id: "grid-test".into(),
                shell_type: ShellType::Windows,
                cwd: None,
                rows: 24,
                cols: 80,
                env: None,
            })
            .expect("CreateSession failed");
        assert!(matches!(response, Response::SessionCreated { .. }));

        // Attach
        let response = fixture
            .send_request(&Request::Attach {
                session_id: "grid-test".into(),
            })
            .expect("Attach failed");
        assert!(
            matches!(response, Response::Ok | Response::Buffer { .. }),
            "Unexpected attach response: {:?}",
            response
        );

        // Write some input
        fixture
            .send_request(&Request::Write {
                session_id: "grid-test".into(),
                data: b"echo hello\r".to_vec(),
            })
            .expect("Write failed");

        // Wait for shell to process
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Read grid
        let response = fixture
            .send_request(&Request::ReadRichGrid {
                session_id: "grid-test".into(),
            })
            .expect("ReadRichGrid failed");

        match response {
            Response::RichGrid { grid } => {
                assert_eq!(grid.dimensions.rows, 24);
                assert_eq!(grid.dimensions.cols, 80);
                assert!(!grid.rows.is_empty(), "Grid should have rows");
            }
            other => panic!("Expected RichGrid, got: {:?}", other),
        }
    }

    #[test]
    fn resize() {
        let mut fixture =
            DaemonFixture::start("resize").expect("Failed to start daemon fixture");

        // Create and attach
        fixture
            .send_request(&Request::CreateSession {
                id: "resize-test".into(),
                shell_type: ShellType::Windows,
                cwd: None,
                rows: 24,
                cols: 80,
                env: None,
            })
            .expect("CreateSession failed");

        fixture
            .send_request(&Request::Attach {
                session_id: "resize-test".into(),
            })
            .expect("Attach failed");

        // Resize
        let response = fixture
            .send_request(&Request::Resize {
                session_id: "resize-test".into(),
                rows: 40,
                cols: 120,
            })
            .expect("Resize failed");
        assert!(
            matches!(response, Response::Ok),
            "Expected Ok, got: {:?}",
            response
        );

        // Verify grid dimensions changed
        std::thread::sleep(std::time::Duration::from_millis(200));

        let response = fixture
            .send_request(&Request::ReadRichGrid {
                session_id: "resize-test".into(),
            })
            .expect("ReadRichGrid failed");

        match response {
            Response::RichGrid { grid } => {
                assert_eq!(grid.dimensions.rows, 40);
                assert_eq!(grid.dimensions.cols, 120);
            }
            other => panic!("Expected RichGrid, got: {:?}", other),
        }
    }

    #[test]
    fn detach_reattach() {
        let mut fixture =
            DaemonFixture::start("detach_reattach").expect("Failed to start daemon fixture");

        // Create and attach
        fixture
            .send_request(&Request::CreateSession {
                id: "detach-test".into(),
                shell_type: ShellType::Windows,
                cwd: None,
                rows: 24,
                cols: 80,
                env: None,
            })
            .expect("CreateSession failed");

        fixture
            .send_request(&Request::Attach {
                session_id: "detach-test".into(),
            })
            .expect("Attach failed");

        // Detach
        let response = fixture
            .send_request(&Request::Detach {
                session_id: "detach-test".into(),
            })
            .expect("Detach failed");
        assert!(
            matches!(response, Response::Ok),
            "Expected Ok, got: {:?}",
            response
        );

        // Re-attach — should get buffer replay
        let response = fixture
            .send_request(&Request::Attach {
                session_id: "detach-test".into(),
            })
            .expect("Re-attach failed");
        assert!(
            matches!(response, Response::Ok | Response::Buffer { .. }),
            "Expected Ok or Buffer, got: {:?}",
            response
        );

        // Verify session is still usable
        let response = fixture
            .send_request(&Request::ReadRichGrid {
                session_id: "detach-test".into(),
            })
            .expect("ReadRichGrid after reattach failed");
        assert!(
            matches!(response, Response::RichGrid { .. }),
            "Expected RichGrid, got: {:?}",
            response
        );
    }
}
