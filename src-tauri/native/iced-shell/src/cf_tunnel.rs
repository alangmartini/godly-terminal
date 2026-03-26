use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

// ---------------------------------------------------------------------------
// Persisted preferences
// ---------------------------------------------------------------------------

const PREFS_FILE_NAME: &str = "cf-tunnel-prefs.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CfTunnelMode {
    Quick,
    Named,
}

impl Default for CfTunnelMode {
    fn default() -> Self {
        Self::Quick
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CfTunnelPreferences {
    pub mode: CfTunnelMode,
    pub tunnel_name: String,
    pub hostname: String,
    pub api_token: String,
    pub account_id: String,
    pub access_email: String,
    /// Stored after Access app creation so we can update/remove it later.
    pub access_app_id: String,
}

impl Default for CfTunnelPreferences {
    fn default() -> Self {
        Self {
            mode: CfTunnelMode::Quick,
            tunnel_name: String::new(),
            hostname: String::new(),
            api_token: String::new(),
            account_id: String::new(),
            access_email: String::new(),
            access_app_id: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Runtime status (not persisted)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CfTunnelStatus {
    Stopped,
    Starting,
    Running,
    Failed(String),
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

fn prefs_path() -> PathBuf {
    let base = std::env::var("APPDATA")
        .ok()
        .or_else(|| std::env::var("HOME").ok())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let directory_name = format!("com.godly.terminal{}", godly_protocol::instance_suffix());
    base.join(directory_name)
        .join("native")
        .join(PREFS_FILE_NAME)
}

pub fn load_preferences() -> CfTunnelPreferences {
    let path = prefs_path();
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => CfTunnelPreferences::default(),
    }
}

pub fn save_preferences(prefs: &CfTunnelPreferences) {
    let path = prefs_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match serde_json::to_string_pretty(prefs) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                log::warn!("Failed to save CF tunnel prefs: {e}");
            }
        }
        Err(e) => log::warn!("Failed to serialize CF tunnel prefs: {e}"),
    }
}

// ---------------------------------------------------------------------------
// Binary discovery
// ---------------------------------------------------------------------------

/// Locate the `cloudflared` binary on the system.
pub fn find_cloudflared() -> Option<PathBuf> {
    // 1. Check PATH
    if let Ok(output) = Command::new("where")
        .arg("cloudflared")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
    {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = stdout.lines().next() {
                let p = PathBuf::from(line.trim());
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    // 2. Common install locations on Windows
    let candidates = [
        r"C:\Program Files\cloudflared\cloudflared.exe",
        r"C:\Program Files (x86)\cloudflared\cloudflared.exe",
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p);
        }
    }

    // 3. Check via winget install path
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(&local)
            .join("Microsoft")
            .join("WinGet")
            .join("Links")
            .join("cloudflared.exe");
        if p.exists() {
            return Some(p);
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Process management — Quick Tunnel
// ---------------------------------------------------------------------------

/// Spawn a quick tunnel (`cloudflared tunnel --url http://localhost:PORT`).
///
/// Returns the child process. The caller must read stderr to find the public
/// URL (see `parse_quick_tunnel_url`).
pub fn spawn_quick_tunnel(cloudflared: &PathBuf, local_port: u16) -> Result<Child, String> {
    let mut cmd = Command::new(cloudflared);
    cmd.args(["tunnel", "--url", &format!("http://localhost:{local_port}")])
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.spawn()
        .map_err(|e| format!("Failed to spawn cloudflared quick tunnel: {e}"))
}

/// Parse the public URL from a line of cloudflared stderr output.
///
/// Quick tunnels print a line like:
/// `INF |  https://random-words.trycloudflare.com  |`
/// or simply:
/// `… https://something.trycloudflare.com …`
pub fn parse_quick_tunnel_url(line: &str) -> Option<String> {
    // Look for a https URL containing trycloudflare.com
    for word in line.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| c == '|' || c == ' ');
        if trimmed.starts_with("https://") && trimmed.contains("trycloudflare.com") {
            return Some(trimmed.to_string());
        }
    }
    None
}

/// Read stderr of a quick tunnel process in a background thread.
/// Returns the public URL when found via the channel.
pub fn read_quick_tunnel_url(
    stderr: std::process::ChildStderr,
) -> futures_channel::oneshot::Receiver<String> {
    let (tx, rx) = futures_channel::oneshot::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    log::debug!("cloudflared: {l}");
                    if let Some(url) = parse_quick_tunnel_url(&l) {
                        let _ = tx.send(url);
                        return;
                    }
                }
                Err(e) => {
                    log::warn!("cloudflared stderr read error: {e}");
                    break;
                }
            }
        }
        // If we never found a URL, the channel just drops
    });
    rx
}

// ---------------------------------------------------------------------------
// Process management — Named Tunnel
// ---------------------------------------------------------------------------

/// Spawn a named tunnel (`cloudflared tunnel run NAME`).
pub fn spawn_named_tunnel(cloudflared: &PathBuf, tunnel_name: &str) -> Result<Child, String> {
    let mut cmd = Command::new(cloudflared);
    cmd.args(["tunnel", "run", tunnel_name])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd.spawn()
        .map_err(|e| format!("Failed to spawn cloudflared named tunnel: {e}"))
}

// ---------------------------------------------------------------------------
// cloudflared CLI operations (blocking — call from background thread)
// ---------------------------------------------------------------------------

/// Run `cloudflared tunnel login`. This opens a browser for authentication.
/// Blocks until the user completes login or the process exits.
pub fn cloudflared_login(cloudflared: &PathBuf) -> Result<(), String> {
    let output = Command::new(cloudflared)
        .args(["tunnel", "login"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .map_err(|e| format!("Failed to run cloudflared login: {e}"))?;

    if output.success() {
        Ok(())
    } else {
        Err("cloudflared login failed or was cancelled".to_string())
    }
}

/// Run `cloudflared tunnel create NAME`. Returns Ok on success.
pub fn create_tunnel(cloudflared: &PathBuf, name: &str) -> Result<(), String> {
    let output = Command::new(cloudflared)
        .args(["tunnel", "create", name])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run cloudflared tunnel create: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "already exists" is not an error for our purposes
        if stderr.contains("already exists") {
            Ok(())
        } else {
            Err(format!(
                "cloudflared tunnel create failed: {}",
                stderr.trim()
            ))
        }
    }
}

/// Run `cloudflared tunnel route dns NAME HOSTNAME`.
pub fn route_dns(cloudflared: &PathBuf, name: &str, hostname: &str) -> Result<(), String> {
    let output = Command::new(cloudflared)
        .args(["tunnel", "route", "dns", name, hostname])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("Failed to run cloudflared tunnel route dns: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // "already exists" is not an error
        if stderr.contains("already exists") {
            Ok(())
        } else {
            Err(format!(
                "cloudflared tunnel route dns failed: {}",
                stderr.trim()
            ))
        }
    }
}

/// Check if `cloudflared` is logged in by listing tunnels.
pub fn check_login_status(cloudflared: &PathBuf) -> bool {
    Command::new(cloudflared)
        .args(["tunnel", "list"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Stop tunnel
// ---------------------------------------------------------------------------

pub fn stop_tunnel(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

// ---------------------------------------------------------------------------
// Cloudflare Access API
// ---------------------------------------------------------------------------

/// Create a Cloudflare Access Application + email-only policy.
///
/// Uses `ureq` (blocking HTTP) — call from a background thread.
/// Returns the Access Application ID on success.
pub fn setup_cloudflare_access(
    api_token: &str,
    account_id: &str,
    hostname: &str,
    email: &str,
) -> Result<String, String> {
    let base = format!("https://api.cloudflare.com/client/v4/accounts/{account_id}/access/apps");

    // 1. Create Access Application
    let app_body = serde_json::json!({
        "name": "Godly Terminal Remote",
        "domain": hostname,
        "type": "self_hosted",
        "session_duration": "24h",
    });

    let resp = ureq::post(&base)
        .set("Authorization", &format!("Bearer {api_token}"))
        .send_json(app_body)
        .map_err(|e| format!("Failed to create Access app: {e}"))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse Access app response: {e}"))?;

    if !body["success"].as_bool().unwrap_or(false) {
        let errors = &body["errors"];
        return Err(format!("Cloudflare API error: {errors}"));
    }

    let app_id = body["result"]["id"]
        .as_str()
        .ok_or_else(|| "Missing app ID in response".to_string())?
        .to_string();

    // 2. Create email-allow policy
    let policy_url = format!("{base}/{app_id}/policies");
    let policy_body = serde_json::json!({
        "name": "Email Access",
        "decision": "allow",
        "include": [
            {
                "email": {
                    "email": email
                }
            }
        ],
    });

    let resp = ureq::post(&policy_url)
        .set("Authorization", &format!("Bearer {api_token}"))
        .send_json(policy_body)
        .map_err(|e| format!("Failed to create Access policy: {e}"))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse Access policy response: {e}"))?;

    if !body["success"].as_bool().unwrap_or(false) {
        let errors = &body["errors"];
        return Err(format!("Cloudflare API error creating policy: {errors}"));
    }

    Ok(app_id)
}

/// Remove a Cloudflare Access Application (and its policies).
pub fn remove_cloudflare_access(
    api_token: &str,
    account_id: &str,
    app_id: &str,
) -> Result<(), String> {
    let url =
        format!("https://api.cloudflare.com/client/v4/accounts/{account_id}/access/apps/{app_id}");

    let resp = ureq::delete(&url)
        .set("Authorization", &format!("Bearer {api_token}"))
        .call()
        .map_err(|e| format!("Failed to delete Access app: {e}"))?;

    let body: serde_json::Value = resp
        .into_json()
        .map_err(|e| format!("Failed to parse delete response: {e}"))?;

    if !body["success"].as_bool().unwrap_or(false) {
        let errors = &body["errors"];
        return Err(format!("Cloudflare API error deleting app: {errors}"));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_preferences() {
        let prefs = CfTunnelPreferences::default();
        assert_eq!(prefs.mode, CfTunnelMode::Quick);
        assert!(prefs.tunnel_name.is_empty());
        assert!(prefs.hostname.is_empty());
        assert!(prefs.api_token.is_empty());
        assert!(prefs.account_id.is_empty());
        assert!(prefs.access_email.is_empty());
        assert!(prefs.access_app_id.is_empty());
    }

    #[test]
    fn preferences_roundtrip_serde() {
        let prefs = CfTunnelPreferences {
            mode: CfTunnelMode::Named,
            tunnel_name: "my-tunnel".to_string(),
            hostname: "phone.example.com".to_string(),
            api_token: "token123".to_string(),
            account_id: "acc456".to_string(),
            access_email: "alan@example.com".to_string(),
            access_app_id: "app789".to_string(),
        };
        let json = serde_json::to_string(&prefs).unwrap();
        let loaded: CfTunnelPreferences = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.mode, CfTunnelMode::Named);
        assert_eq!(loaded.tunnel_name, "my-tunnel");
        assert_eq!(loaded.hostname, "phone.example.com");
        assert_eq!(loaded.api_token, "token123");
        assert_eq!(loaded.account_id, "acc456");
        assert_eq!(loaded.access_email, "alan@example.com");
        assert_eq!(loaded.access_app_id, "app789");
    }

    #[test]
    fn missing_fields_use_defaults() {
        let json = r#"{"tunnel_name": "test"}"#;
        let loaded: CfTunnelPreferences = serde_json::from_str(json).unwrap();
        assert_eq!(loaded.mode, CfTunnelMode::Quick);
        assert_eq!(loaded.tunnel_name, "test");
        assert!(loaded.hostname.is_empty());
    }

    #[test]
    fn parse_quick_tunnel_url_from_box_line() {
        let line = "2024-01-01T00:00:00Z INF |  https://random-words.trycloudflare.com                                                    |";
        assert_eq!(
            parse_quick_tunnel_url(line),
            Some("https://random-words.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn parse_quick_tunnel_url_from_plain_line() {
        let line = "2024-01-01T00:00:00Z INF https://test-hello.trycloudflare.com";
        assert_eq!(
            parse_quick_tunnel_url(line),
            Some("https://test-hello.trycloudflare.com".to_string())
        );
    }

    #[test]
    fn parse_quick_tunnel_url_no_match() {
        let line = "2024-01-01T00:00:00Z INF Starting tunnel";
        assert_eq!(parse_quick_tunnel_url(line), None);
    }

    #[test]
    fn mode_serde() {
        let q = serde_json::to_string(&CfTunnelMode::Quick).unwrap();
        assert_eq!(q, r#""quick""#);
        let n = serde_json::to_string(&CfTunnelMode::Named).unwrap();
        assert_eq!(n, r#""named""#);
    }
}
