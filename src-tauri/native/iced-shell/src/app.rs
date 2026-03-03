use std::sync::Arc;
use std::time::Duration;

use iced::keyboard;
use iced::widget::{canvas, center, text};
use iced::{Element, Length, Subscription, Task};

use godly_app_adapter::commands;
use godly_app_adapter::daemon_client::{FrontendEventSink, NativeDaemonClient};
use godly_app_adapter::keys::key_to_pty_bytes;
use godly_protocol::types::RichGridData;

use godly_terminal_surface::{TerminalCanvas, TerminalCanvasState};

use crate::subscription::DaemonEventMsg;

/// Main Iced application state.
pub struct GodlyApp {
    /// Daemon client (shared with bridge thread).
    client: Option<Arc<NativeDaemonClient>>,
    /// Current session ID.
    session_id: Option<String>,
    /// Terminal canvas state (shared with Canvas Program).
    canvas_state: TerminalCanvasState,
    /// Window title from terminal.
    terminal_title: String,
    /// Error message to display if initialization failed.
    init_error: Option<String>,
    /// Grid dimensions in cells.
    grid_rows: u16,
    grid_cols: u16,
    /// Whether a grid fetch is currently in flight.
    fetching_grid: bool,
    /// Pending output flag — set by event sink, cleared by grid fetch.
    has_pending_output: Arc<std::sync::atomic::AtomicBool>,
}

impl Default for GodlyApp {
    fn default() -> Self {
        Self {
            client: None,
            session_id: None,
            canvas_state: TerminalCanvasState::default(),
            terminal_title: String::new(),
            init_error: None,
            grid_rows: 24,
            grid_cols: 80,
            fetching_grid: false,
            has_pending_output: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }
}

/// Application messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// Daemon event received from bridge thread.
    DaemonEvent(DaemonEventMsg),
    /// Grid snapshot fetched after output event.
    GridFetched(RichGridData),
    /// Grid fetch failed (log only).
    GridFetchFailed(String),
    /// Keyboard event from iced.
    KeyboardEvent(keyboard::Event),
    /// Initialization complete.
    Initialized(Result<InitResult, String>),
    /// Timer tick for polling daemon events.
    Tick,
}

#[derive(Debug, Clone)]
pub struct InitResult {
    pub session_id: String,
}

/// Event sink implementation that sets an atomic flag.
/// The iced app polls this flag on timer ticks.
struct AtomicEventSink {
    has_output: Arc<std::sync::atomic::AtomicBool>,
}

impl FrontendEventSink for AtomicEventSink {
    fn on_terminal_output(&self, _session_id: &str) {
        self.has_output
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn on_session_closed(&self, _session_id: &str, _exit_code: Option<i64>) {
        self.has_output
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn on_process_changed(&self, _session_id: &str, _process_name: &str) {
        self.has_output
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn on_grid_diff(&self, _session_id: &str, _diff_bytes: &[u8]) {
        self.has_output
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    fn on_bell(&self, _session_id: &str) {}
}

impl GodlyApp {
    pub fn title(&self) -> String {
        if !self.terminal_title.is_empty() {
            format!("{} — Godly Terminal (Native)", self.terminal_title)
        } else {
            format!(
                "Godly Terminal (Native) — contract v{}",
                godly_protocol::FRONTEND_CONTRACT_VERSION
            )
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Initialized(Ok(result)) => {
                self.session_id = Some(result.session_id);
                return self.fetch_grid();
            }
            Message::Initialized(Err(e)) => {
                log::error!("Initialization failed: {}", e);
                self.init_error = Some(e);
            }
            Message::Tick => {
                // Poll for pending output from the daemon event sink
                if self
                    .has_pending_output
                    .swap(false, std::sync::atomic::Ordering::Relaxed)
                    && !self.fetching_grid
                {
                    return self.fetch_grid();
                }
            }
            Message::DaemonEvent(_) => {}
            Message::GridFetched(grid) => {
                self.fetching_grid = false;
                self.terminal_title = grid.title.clone();
                self.canvas_state.grid = Some(grid);
            }
            Message::GridFetchFailed(e) => {
                self.fetching_grid = false;
                log::error!("Grid fetch failed: {}", e);
            }
            Message::KeyboardEvent(keyboard::Event::KeyPressed {
                key, modifiers, ..
            }) => {
                if let Some(bytes) = key_to_pty_bytes(&key, modifiers) {
                    if let (Some(client), Some(sid)) = (&self.client, &self.session_id) {
                        let _ = commands::write_to_terminal(client, sid, &bytes);
                    }
                }
            }
            Message::KeyboardEvent(_) => {}
        }
        Task::none()
    }

    pub fn view(&self) -> Element<'_, Message> {
        if let Some(ref err) = self.init_error {
            return center(text(format!("Initialization error: {}", err)).size(18)).into();
        }

        if self.session_id.is_none() && self.client.is_none() {
            return center(text("Connecting to daemon...").size(18)).into();
        }

        if self.session_id.is_none() {
            return center(text("Session closed").size(18)).into();
        }

        canvas(TerminalCanvas)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            // Keyboard events
            keyboard::listen().map(Message::KeyboardEvent),
            // Timer for polling daemon events (~16ms = 60fps)
            iced::time::every(Duration::from_millis(16)).map(|_| Message::Tick),
        ])
    }

    /// Spawn a background task to fetch the grid snapshot.
    fn fetch_grid(&mut self) -> Task<Message> {
        if let (Some(client), Some(sid)) = (&self.client, &self.session_id) {
            self.fetching_grid = true;
            let client = Arc::clone(client);
            let sid = sid.clone();
            Task::perform(
                async move {
                    let (tx, rx) = futures_channel::oneshot::channel();
                    std::thread::spawn(move || {
                        let result = commands::get_grid_snapshot(&client, &sid);
                        let _ = tx.send(result);
                    });
                    rx.await
                        .unwrap_or_else(|_| Err("Background thread panicked".into()))
                },
                |result| match result {
                    Ok(grid) => Message::GridFetched(grid),
                    Err(e) => Message::GridFetchFailed(e),
                },
            )
        } else {
            Task::none()
        }
    }
}

/// Initialize the app: connect to daemon, set up bridge, create session.
pub fn initialize(app: &mut GodlyApp) -> Task<Message> {
    // Connect to daemon
    let client = match NativeDaemonClient::connect_or_launch() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            return Task::done(Message::Initialized(Err(e)));
        }
    };

    // Set up bridge with atomic flag event sink
    let sink = Arc::new(AtomicEventSink {
        has_output: Arc::clone(&app.has_pending_output),
    });
    if let Err(e) = client.setup_bridge(sink) {
        return Task::done(Message::Initialized(Err(e)));
    }

    app.client = Some(Arc::clone(&client));

    // Create a session
    let session_id = uuid::Uuid::new_v4().to_string();
    let sid = session_id.clone();
    let rows = app.grid_rows;
    let cols = app.grid_cols;

    Task::perform(
        async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            std::thread::spawn(move || {
                let result = commands::create_terminal(
                    &client,
                    &sid,
                    godly_protocol::ShellType::Windows,
                    None,
                    rows,
                    cols,
                )
                .map(|_| InitResult {
                    session_id: sid,
                });
                let _ = tx.send(result);
            });
            rx.await
                .unwrap_or_else(|_| Err("Background thread panicked".into()))
        },
        Message::Initialized,
    )
}
