use godly_protocol::messages::{DaemonMessage, Event, Request, Response};
use godly_protocol::types::*;
use godly_protocol::FRONTEND_CONTRACT_VERSION;
use godly_parity_harness::snapshot::GridSnapshotComparator;

#[test]
fn contract_version_is_frozen() {
    assert_eq!(FRONTEND_CONTRACT_VERSION, "1.0.0");
}

#[test]
fn request_create_session_roundtrip() {
    let req = Request::CreateSession {
        id: "test-sess".into(),
        shell_type: ShellType::Windows,
        cwd: Some("C:\\Users\\test".into()),
        rows: 24,
        cols: 80,
        env: None,
    };
    let json = serde_json::to_string(&req).unwrap();
    let decoded: Request = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded, Request::CreateSession { id, .. } if id == "test-sess"));
}

#[test]
fn request_all_variants_serialize() {
    let variants: Vec<Request> = vec![
        Request::CreateSession {
            id: "s".into(), shell_type: ShellType::Pwsh, cwd: None, rows: 24, cols: 80, env: None,
        },
        Request::ListSessions,
        Request::Attach { session_id: "s".into() },
        Request::Detach { session_id: "s".into() },
        Request::CloseSession { session_id: "s".into() },
        Request::Write { session_id: "s".into(), data: vec![65] },
        Request::Resize { session_id: "s".into(), rows: 30, cols: 120 },
        Request::ReadBuffer { session_id: "s".into() },
        Request::ReadGrid { session_id: "s".into() },
        Request::ReadRichGrid { session_id: "s".into() },
        Request::ReadRichGridDiff { session_id: "s".into() },
        Request::ReadGridText {
            session_id: "s".into(), start_row: 0, start_col: 0,
            end_row: 10, end_col: 80, scrollback_offset: 0,
        },
        Request::SetScrollback { session_id: "s".into(), offset: 5 },
        Request::ScrollAndReadRichGrid { session_id: "s".into(), offset: 10 },
        Request::GetLastOutputTime { session_id: "s".into() },
        Request::SearchBuffer { session_id: "s".into(), text: "hello".into(), strip_ansi: true },
        Request::PauseSession { session_id: "s".into() },
        Request::ResumeSession { session_id: "s".into() },
        Request::Ping,
    ];
    for variant in &variants {
        let json = serde_json::to_string(variant).unwrap();
        let _: Request = serde_json::from_str(&json).unwrap();
    }
    assert_eq!(variants.len(), 19, "Update when adding new Request variants");
}

#[test]
fn response_all_variants_serialize() {
    let variants: Vec<Response> = vec![
        Response::Ok,
        Response::Error { message: "err".into() },
        Response::SessionCreated {
            session: SessionInfo {
                id: "s".into(), shell_type: ShellType::Windows, pid: 1234,
                rows: 24, cols: 80, cwd: None, created_at: 1700000000,
                attached: true, running: true, scrollback_rows: 0,
                scrollback_memory_bytes: 0, paused: false, title: String::new(),
            },
        },
        Response::SessionList { sessions: vec![] },
        Response::Buffer { session_id: "s".into(), data: vec![65] },
        Response::Grid {
            grid: GridData {
                rows: vec!["hello".into()], cursor_row: 0, cursor_col: 5,
                cols: 80, num_rows: 24, alternate_screen: false,
            },
        },
        Response::RichGrid { grid: make_test_rich_grid() },
        Response::RichGridDiff { diff: make_test_rich_grid_diff() },
        Response::GridText { text: "hello world".into() },
        Response::LastOutputTime {
            epoch_ms: 1700000000000, running: true, exit_code: None, input_expected: None,
        },
        Response::SearchResult { found: true, running: true },
        Response::Pong,
    ];
    for variant in &variants {
        let json = serde_json::to_string(variant).unwrap();
        let _: Response = serde_json::from_str(&json).unwrap();
    }
    assert_eq!(variants.len(), 12, "Update when adding new Response variants");
}

#[test]
fn event_all_variants_serialize() {
    let variants: Vec<Event> = vec![
        Event::Output { session_id: "s".into(), data: vec![65] },
        Event::SessionClosed { session_id: "s".into(), exit_code: Some(0) },
        Event::ProcessChanged { session_id: "s".into(), process_name: "bash".into() },
        Event::GridDiff { session_id: "s".into(), diff: make_test_rich_grid_diff() },
        Event::Bell { session_id: "s".into() },
    ];
    for variant in &variants {
        let json = serde_json::to_string(variant).unwrap();
        let _: Event = serde_json::from_str(&json).unwrap();
    }
    assert_eq!(variants.len(), 5, "Update when adding new Event variants");
}

#[test]
fn daemon_message_roundtrip() {
    let msg = DaemonMessage::Response(Response::Pong);
    let json = serde_json::to_string(&msg).unwrap();
    let decoded: DaemonMessage = serde_json::from_str(&json).unwrap();
    assert!(matches!(decoded, DaemonMessage::Response(Response::Pong)));
}

#[test]
fn frontend_mode_variants() {
    for mode in [FrontendMode::Web, FrontendMode::Native, FrontendMode::Shadow] {
        let json = serde_json::to_string(&mode).unwrap();
        let decoded: FrontendMode = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, mode);
    }
}

#[test]
fn grid_comparator_identical_snapshots() {
    let grid = make_test_rich_grid();
    let result = GridSnapshotComparator::compare(&grid, &grid);
    assert!(result.is_identical());
}

#[test]
fn grid_comparator_detects_content_mismatch() {
    let grid_a = make_test_rich_grid();
    let mut grid_b = grid_a.clone();
    grid_b.rows[0].cells[0].content = "X".into();
    let result = GridSnapshotComparator::compare(&grid_a, &grid_b);
    assert!(!result.is_identical());
    assert_eq!(result.mismatches[0].field, "content");
}

#[test]
fn grid_comparator_detects_cursor_mismatch() {
    let grid_a = make_test_rich_grid();
    let mut grid_b = grid_a.clone();
    grid_b.cursor.col = 42;
    let result = GridSnapshotComparator::compare(&grid_a, &grid_b);
    assert!(!result.is_identical());
    assert!(result.mismatches.iter().any(|m| m.field == "cursor"));
}

fn make_test_cell(ch: &str) -> RichGridCell {
    RichGridCell {
        content: ch.into(), fg: "default".into(), bg: "default".into(),
        bold: false, dim: false, italic: false, underline: false,
        inverse: false, wide: false, wide_continuation: false,
    }
}

fn make_test_rich_grid() -> RichGridData {
    RichGridData {
        rows: vec![RichGridRow { cells: vec![make_test_cell("H"), make_test_cell("i")], wrapped: false }],
        cursor: CursorState { row: 0, col: 2 },
        dimensions: GridDimensions { rows: 24, cols: 80 },
        alternate_screen: false, cursor_hidden: false, title: String::new(),
        scrollback_offset: 0, total_scrollback: 0,
    }
}

fn make_test_rich_grid_diff() -> RichGridDiff {
    RichGridDiff {
        dirty_rows: vec![], cursor: CursorState { row: 0, col: 0 },
        dimensions: GridDimensions { rows: 24, cols: 80 },
        alternate_screen: false, cursor_hidden: false, title: String::new(),
        scrollback_offset: 0, total_scrollback: 0, full_repaint: false,
    }
}
