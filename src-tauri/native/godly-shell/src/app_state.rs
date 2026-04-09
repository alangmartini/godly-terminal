use std::collections::HashMap;
use std::sync::Arc;

use godly_app_adapter::daemon_client::NativeDaemonClient;
use godly_layout_core::LayoutNode;
use godly_protocol::messages::{Request, Response};
use godly_protocol::types::{RichGridData, ShellType};
use godly_tabs_core::TabState;
use godly_terminal_surface::font_metrics::FontMetrics;
use godly_workspaces_core::WorkspaceCollection;

use crate::event_bus::EventSender;

/// Central application state.
pub struct AppState {
    pub client: Arc<NativeDaemonClient>,
    pub sender: EventSender,
    pub tabs: TabState,
    pub grids: HashMap<String, RichGridData>,
    pub focused_terminal: Option<String>,
    pub font_metrics: FontMetrics,
    pub sidebar_visible: bool,
}

impl AppState {
    pub fn new(
        client: Arc<NativeDaemonClient>,
        sender: EventSender,
        font_metrics: FontMetrics,
    ) -> Self {
        Self {
            client,
            sender,
            tabs: TabState::new(),
            grids: HashMap::new(),
            focused_terminal: None,
            font_metrics,
            sidebar_visible: false,
        }
    }

    pub fn create_tab(&mut self, rows: u16, cols: u16) -> Option<String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        match self.client.send_request(&Request::CreateSession {
            id: session_id.clone(),
            shell_type: ShellType::Windows,
            cwd: None,
            rows,
            cols,
            env: None,
        }) {
            Ok(Response::SessionCreated { session }) => {
                self.tabs.open(session.id.clone());
                self.tabs.activate(&session.id);
                self.focused_terminal = Some(session.id.clone());
                log::info!("Created tab: {}", session.id);
                Some(session.id)
            }
            Ok(other) => {
                log::error!("Unexpected response creating session: {other:?}");
                None
            }
            Err(e) => {
                log::error!("Failed to create session: {e}");
                None
            }
        }
    }

    pub fn close_tab(&mut self, session_id: &str) {
        let client = Arc::clone(&self.client);
        let id = session_id.to_string();
        std::thread::spawn(move || {
            let _ = client.send_request(&Request::CloseSession { session_id: id });
        });
        self.tabs.close(session_id);
        self.grids.remove(session_id);
        // Focus next available tab
        if self.focused_terminal.as_deref() == Some(session_id) {
            self.focused_terminal = self.tabs.active_id().map(|s| s.to_string());
        }
    }

    pub fn switch_tab(&mut self, session_id: &str) {
        self.tabs.activate(session_id);
        self.focused_terminal = Some(session_id.to_string());
    }
}
