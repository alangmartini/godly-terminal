use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_server")]
    pub server: ServerConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub monitor: MonitorConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AuthConfig {
    #[serde(default)]
    pub api_key: Option<String>,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self { api_key: None }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct MonitorConfig {
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
    #[serde(default)]
    pub webhook_url: Option<String>,
    #[serde(default = "default_scan_rows")]
    pub scan_rows: usize,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval_ms: default_poll_interval(),
            webhook_url: None,
            scan_rows: default_scan_rows(),
        }
    }
}

fn default_server() -> ServerConfig {
    ServerConfig {
        host: default_host(),
        port: default_port(),
    }
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3377
}

fn default_poll_interval() -> u64 {
    1000
}

fn default_scan_rows() -> usize {
    10
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: default_server(),
            auth: AuthConfig { api_key: None },
            monitor: MonitorConfig {
                poll_interval_ms: default_poll_interval(),
                webhook_url: None,
                scan_rows: default_scan_rows(),
            },
        }
    }
}

impl Config {
    /// Load config from TOML file, falling back to defaults.
    /// Env vars override file values:
    ///   GODLY_REMOTE_API_KEY -> auth.api_key
    ///   GODLY_REMOTE_HOST -> server.host
    ///   GODLY_REMOTE_PORT -> server.port
    ///   GODLY_REMOTE_WEBHOOK_URL -> monitor.webhook_url
    pub fn load() -> Self {
        let mut config = Self::load_from_file();

        if let Ok(key) = std::env::var("GODLY_REMOTE_API_KEY") {
            config.auth.api_key = Some(key);
        }
        if let Ok(host) = std::env::var("GODLY_REMOTE_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("GODLY_REMOTE_PORT") {
            if let Ok(p) = port.parse() {
                config.server.port = p;
            }
        }
        if let Ok(url) = std::env::var("GODLY_REMOTE_WEBHOOK_URL") {
            config.monitor.webhook_url = Some(url);
        }

        config
    }

    fn load_from_file() -> Self {
        let candidates = config_file_candidates();
        for path in candidates {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                match toml::from_str(&contents) {
                    Ok(config) => {
                        tracing::info!("Loaded config from {}", path.display());
                        return config;
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse {}: {}", path.display(), e);
                    }
                }
            }
        }
        tracing::info!("No config file found, using defaults");
        Config::default()
    }
}

fn config_file_candidates() -> Vec<PathBuf> {
    let mut paths = vec![PathBuf::from("godly-remote.toml")];
    if let Ok(appdata) = std::env::var("APPDATA") {
        paths.push(
            PathBuf::from(appdata)
                .join("com.godly.terminal")
                .join("godly-remote.toml"),
        );
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_values() {
        let config = Config::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3377);
        assert!(config.auth.api_key.is_none());
        assert_eq!(config.monitor.poll_interval_ms, 1000);
        assert_eq!(config.monitor.scan_rows, 10);
        assert!(config.monitor.webhook_url.is_none());
    }

    #[test]
    fn parse_full_toml() {
        let toml_str = r#"
[server]
host = "127.0.0.1"
port = 8080

[auth]
api_key = "secret123"

[monitor]
poll_interval_ms = 2000
webhook_url = "https://example.com/hook"
scan_rows = 5
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.auth.api_key.as_deref(), Some("secret123"));
        assert_eq!(config.monitor.poll_interval_ms, 2000);
        assert_eq!(
            config.monitor.webhook_url.as_deref(),
            Some("https://example.com/hook")
        );
        assert_eq!(config.monitor.scan_rows, 5);
    }

    #[test]
    fn parse_partial_toml_uses_defaults() {
        let toml_str = r#"
[server]
port = 9000
"#;
        let config: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 9000);
        assert!(config.auth.api_key.is_none());
        assert_eq!(config.monitor.poll_interval_ms, 1000);
    }

    #[test]
    fn parse_empty_toml() {
        let config: Config = toml::from_str("").unwrap();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3377);
    }
}
