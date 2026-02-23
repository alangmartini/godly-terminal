mod auth;
mod config;
mod daemon_client;
mod detection;
mod device_lock;
mod event_pump;
mod layout_reader;
mod monitor;
mod routes;
mod ws;

use std::sync::Arc;

use config::Config;
use daemon_client::DaemonClient;
use device_lock::DeviceLock;
use event_pump::EventPump;
use layout_reader::LayoutReader;
use monitor::MonitorRegistry;
use tower_http::cors::{AllowHeaders, AllowMethods, CorsLayer};

const BUILD: u32 = 4;

/// Shared application state, cloneable via Arc internals.
#[derive(Clone)]
pub struct AppState {
    pub daemon: Arc<DaemonClient>,
    pub config: Arc<Config>,
    pub monitors: Arc<MonitorRegistry>,
    pub layout_reader: Arc<LayoutReader>,
    pub event_pump: Arc<EventPump>,
    pub device_lock: Arc<DeviceLock>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "godly_remote=info".into()),
        )
        .init();

    tracing::info!("=== godly-remote starting === build={}", BUILD);

    let config = Config::load();

    if config.auth.api_key.is_none() {
        tracing::warn!("No API key configured — running in dev mode (all requests allowed)");
    }

    // Warn if binding to all interfaces without auth
    if config.server.host == "0.0.0.0" && config.auth.api_key.is_none() {
        tracing::warn!(
            "WARNING: Binding to 0.0.0.0 without API key — server is accessible to anyone on the network!"
        );
    }

    // Connect to daemon
    let daemon = match DaemonClient::connect() {
        Ok(d) => d,
        Err(e) => {
            tracing::error!("Failed to connect to daemon: {}", e);
            tracing::error!("Make sure the daemon is running (npm run build:daemon && godly-daemon)");
            std::process::exit(1);
        }
    };

    tracing::info!("Connected to daemon");

    let api_key = config.auth.api_key.clone();
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);
    let scan_rows = config.monitor.scan_rows;

    let daemon = Arc::new(daemon);
    let event_pump = Arc::new(EventPump::new());

    // Start the SSE event pump background task
    event_pump.spawn(Arc::clone(&daemon), scan_rows);
    tracing::info!("Event pump started (scan_rows={})", scan_rows);

    let auth_password = std::env::var("GODLY_REMOTE_PASSWORD").ok();
    if auth_password.is_some() {
        tracing::info!("Device registration requires password");
    }
    let device_lock = Arc::new(DeviceLock::new(auth_password));

    let state = AppState {
        daemon,
        config: Arc::new(config),
        monitors: Arc::new(MonitorRegistry::new()),
        layout_reader: Arc::new(LayoutReader::new()),
        event_pump,
        device_lock: Arc::clone(&device_lock),
    };

    // CORS: only allow the same origin (ngrok tunnel or localhost).
    // No wildcard — credentials must come from the served phone.html page.
    let cors = CorsLayer::new()
        .allow_methods(AllowMethods::mirror_request())
        .allow_headers(AllowHeaders::mirror_request())
        .allow_credentials(true);

    let app = routes::build_router(state)
        .layer(cors)
        .layer(axum::Extension(device_lock))
        .layer(axum::Extension(api_key));

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to bind to {}: {}", bind_addr, e);
            std::process::exit(1);
        });

    tracing::info!("Listening on http://{}", bind_addr);
    tracing::info!("Phone UI available at http://{}/phone", bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Server error: {}", e);
            std::process::exit(1);
        });

    tracing::info!("Server shut down gracefully");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to listen for Ctrl+C");
    tracing::info!("Received Ctrl+C, shutting down...");
}
