mod auth;
mod config;
mod daemon_client;
mod monitor;
mod routes;
mod ws;

use std::sync::Arc;

use config::Config;
use daemon_client::DaemonClient;
use monitor::MonitorRegistry;

const BUILD: u32 = 1;

/// Shared application state, cloneable via Arc internals.
#[derive(Clone)]
pub struct AppState {
    pub daemon: Arc<DaemonClient>,
    pub config: Arc<Config>,
    pub monitors: Arc<MonitorRegistry>,
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

    let state = AppState {
        daemon: Arc::new(daemon),
        config: Arc::new(config),
        monitors: Arc::new(MonitorRegistry::new()),
    };

    let app = routes::build_router(state)
        .layer(axum::Extension(api_key));

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to bind to {}: {}", bind_addr, e);
            std::process::exit(1);
        });

    tracing::info!("Listening on http://{}", bind_addr);

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
