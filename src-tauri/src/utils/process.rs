/// Get the current working directory of a process by PID
#[cfg(windows)]
pub fn get_process_cwd(pid: u32) -> Option<String> {
    // For getting CWD, we use PowerShell to query the process
    // This is simpler than using the Windows API directly
    get_cwd_via_powershell(pid)
}

#[cfg(windows)]
fn get_cwd_via_powershell(pid: u32) -> Option<String> {
    use std::os::windows::process::CommandExt;
    use std::process::Command;
    use winapi::um::winbase::CREATE_NO_WINDOW;

    // Use Get-Process and WMI to find the working directory
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &format!(
                r#"
                try {{
                    $wmi = Get-WmiObject Win32_Process -Filter "ProcessId = {}" -ErrorAction Stop
                    if ($wmi -and $wmi.ExecutablePath) {{
                        [System.IO.Path]::GetDirectoryName($wmi.ExecutablePath)
                    }}
                }} catch {{
                    # Silently fail
                }}
                "#,
                pid
            ),
        ])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;

    if output.status.success() {
        let cwd = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();
        if !cwd.is_empty() && cwd != "System32" && !cwd.contains("WindowsApps") {
            return Some(cwd);
        }
    }

    None
}

#[cfg(not(windows))]
pub fn get_process_cwd(_pid: u32) -> Option<String> {
    None
}
