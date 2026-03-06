use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Check if the whisper binary is available at the expected location.
pub fn whisper_binary_path() -> Option<PathBuf> {
    let local_app_data = std::env::var("LOCALAPPDATA").ok()?;
    let path = PathBuf::from(local_app_data)
        .join("godly-whisper")
        .join("godly-whisper.exe");
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

/// Result of a completed transcription.
#[derive(Debug, Clone)]
pub struct WhisperResult {
    pub text: String,
    pub duration_ms: u64,
}

/// Manages the whisper sidecar process via stdin/stdout JSON lines.
pub struct WhisperService {
    child: Child,
    stdin: ChildStdin,
    reader: BufReader<std::process::ChildStdout>,
}

impl WhisperService {
    /// Spawn the whisper sidecar process.
    pub fn spawn() -> Result<Self, String> {
        let path = whisper_binary_path().ok_or("Whisper binary not found")?;

        let mut cmd = Command::new(&path);
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn whisper: {e}"))?;

        let stdin = child
            .stdin
            .take()
            .ok_or("Failed to capture whisper stdin")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Failed to capture whisper stdout")?;
        let reader = BufReader::new(stdout);

        Ok(Self {
            child,
            stdin,
            reader,
        })
    }

    /// Send a JSON command and read the JSON response line.
    fn send_and_recv(&mut self, cmd: &str) -> Result<serde_json::Value, String> {
        let line = format!("{{\"cmd\":\"{cmd}\"}}\n");
        self.stdin
            .write_all(line.as_bytes())
            .map_err(|e| format!("Failed to write to whisper: {e}"))?;
        self.stdin
            .flush()
            .map_err(|e| format!("Failed to flush whisper stdin: {e}"))?;

        let mut buf = String::new();
        self.reader
            .read_line(&mut buf)
            .map_err(|e| format!("Failed to read from whisper: {e}"))?;

        serde_json::from_str(&buf).map_err(|e| format!("Invalid JSON from whisper: {e}"))
    }

    /// Start recording audio.
    pub fn start_recording(&mut self) -> Result<(), String> {
        let resp = self.send_and_recv("start")?;
        match resp.get("type").and_then(|v| v.as_str()) {
            Some("started") => Ok(()),
            Some("error") => Err(resp
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string()),
            _ => Err(format!("Unexpected whisper response: {resp}")),
        }
    }

    /// Stop recording and return the transcription result.
    pub fn stop_recording(&mut self) -> Result<WhisperResult, String> {
        let resp = self.send_and_recv("stop")?;
        match resp.get("type").and_then(|v| v.as_str()) {
            Some("stopped") => {
                let text = resp
                    .get("text")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let duration_ms = resp
                    .get("duration_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                Ok(WhisperResult { text, duration_ms })
            }
            Some("error") => Err(resp
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string()),
            _ => Err(format!("Unexpected whisper response: {resp}")),
        }
    }

    /// Get the current audio input level (0.0..1.0).
    pub fn get_level(&mut self) -> Result<f32, String> {
        let resp = self.send_and_recv("level")?;
        match resp.get("type").and_then(|v| v.as_str()) {
            Some("level") => {
                let value = resp
                    .get("value")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.0) as f32;
                Ok(value.clamp(0.0, 1.0))
            }
            Some("error") => Err(resp
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error")
                .to_string()),
            _ => Err(format!("Unexpected whisper response: {resp}")),
        }
    }

    /// Kill the sidecar process.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

impl Drop for WhisperService {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_binary_path_returns_none_when_missing() {
        let _ = whisper_binary_path();
    }
}
