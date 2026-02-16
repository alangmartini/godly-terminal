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
