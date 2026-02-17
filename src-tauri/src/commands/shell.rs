use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use winapi::um::winbase::CREATE_NO_WINDOW;

/// Check if PowerShell 7 (pwsh.exe) is available on the system.
#[tauri::command]
pub fn is_pwsh_available() -> bool {
    #[cfg(windows)]
    {
        Command::new("where.exe")
            .arg("pwsh.exe")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Command::new("which")
            .arg("pwsh")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

/// Return the path to the CMD aliases file: `%USERPROFILE%\cmd-aliases.cmd`.
#[tauri::command]
pub fn get_cmd_aliases_path() -> Result<String, String> {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "Could not determine home directory".to_string())?;
    let path = PathBuf::from(home).join("cmd-aliases.cmd");
    path.to_str()
        .map(|s| s.to_string())
        .ok_or_else(|| "Invalid path encoding".to_string())
}

/// Idempotently ensure the CMD aliases file is registered as the AutoRun
/// script for cmd.exe via the Windows registry.
///
/// Returns a status string: "already_configured", "configured", or "appended".
#[tauri::command]
pub fn ensure_cmd_autorun() -> Result<String, String> {
    #[cfg(windows)]
    {
        let aliases_path = get_cmd_aliases_path()?;

        // Query current AutoRun value
        let query = Command::new("reg")
            .args([
                "query",
                r"HKCU\Software\Microsoft\Command Processor",
                "/v",
                "AutoRun",
            ])
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|e| format!("Failed to query registry: {e}"))?;

        let stdout = String::from_utf8_lossy(&query.stdout);

        if query.status.success() {
            // Parse existing value — format: "    AutoRun    REG_SZ    <value>"
            if let Some(existing) = parse_reg_value(&stdout) {
                // Already contains our path (case-insensitive comparison)
                if existing.to_lowercase().contains(&aliases_path.to_lowercase()) {
                    return Ok("already_configured".to_string());
                }
                // Append to existing value
                let new_value = format!("{existing} & \"{aliases_path}\"");
                reg_set_autorun(&new_value)?;
                return Ok("appended".to_string());
            }
        }

        // No AutoRun value exists — create it
        reg_set_autorun(&format!("\"{aliases_path}\""))?;
        Ok("configured".to_string())
    }

    #[cfg(not(windows))]
    {
        Err("CMD AutoRun is only supported on Windows".to_string())
    }
}

/// Parse the value from `reg query` output.
/// The output format is: `    AutoRun    REG_SZ    <value>`
#[cfg(windows)]
fn parse_reg_value(output: &str) -> Option<String> {
    for line in output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("AutoRun") {
            // Split on REG_SZ and take everything after it
            if let Some(idx) = trimmed.find("REG_SZ") {
                let value = trimmed[idx + "REG_SZ".len()..].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

/// Set the AutoRun registry value.
#[cfg(windows)]
fn reg_set_autorun(value: &str) -> Result<(), String> {
    let status = Command::new("reg")
        .args([
            "add",
            r"HKCU\Software\Microsoft\Command Processor",
            "/v",
            "AutoRun",
            "/t",
            "REG_SZ",
            "/d",
            value,
            "/f",
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| format!("Failed to set registry: {e}"))?;

    if status.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&status.stderr);
        Err(format!("Failed to set AutoRun: {stderr}"))
    }
}
