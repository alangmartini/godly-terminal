use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use std::io::{Read, Write};

/// The result of opening a PTY, split into separately-owned parts
/// so each can be moved to a different thread.
pub struct PtyParts {
    pub master: Box<dyn MasterPty + Send>,
    pub writer: Box<dyn Write + Send>,
    pub reader: Box<dyn Read + Send>,
    pub shell_pid: u32,
    /// Child process handle — used to retrieve the real exit code after the shell exits.
    pub child: Box<dyn Child + Send + Sync>,
}

/// Open a new PTY with the given shell type and dimensions.
///
/// `shell_type` values:
/// - `"windows"` -- PowerShell with `-NoLogo`
/// - `"pwsh"` -- PowerShell Core with `-NoLogo`
/// - `"cmd"` -- cmd.exe
/// - `"wsl"` or `"wsl:Ubuntu"` -- WSL with optional distribution
/// - Any other string -- treated as a custom shell command
pub fn open_pty(
    shell_type: &str,
    cwd: Option<&str>,
    rows: u16,
    cols: u16,
    env: Option<&std::collections::HashMap<String, String>>,
) -> Result<PtyParts, String> {
    let pty_system = native_pty_system();
    let size = PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    };
    let pair = pty_system
        .openpty(size)
        .map_err(|e| format!("openpty: {}", e))?;

    let mut cmd = match shell_type {
        "windows" => {
            let mut c = CommandBuilder::new("powershell.exe");
            c.arg("-NoLogo");
            // Bug #669: Override any profile Set-Location so the workspace CWD sticks.
            // -Command runs AFTER profiles, so it wins over profile cd/Set-Location.
            if let Some(dir) = cwd {
                c.args([
                    "-NoExit",
                    "-Command",
                    &format!("Set-Location -LiteralPath '{}'", dir.replace('\'', "''")),
                ]);
            }
            c
        }
        "pwsh" => {
            let mut c = CommandBuilder::new("pwsh.exe");
            c.arg("-NoLogo");
            if let Some(dir) = cwd {
                c.args([
                    "-NoExit",
                    "-Command",
                    &format!("Set-Location -LiteralPath '{}'", dir.replace('\'', "''")),
                ]);
            }
            c
        }
        "cmd" => CommandBuilder::new("cmd.exe"),
        s if s.starts_with("wsl") => {
            let mut c = CommandBuilder::new("wsl.exe");
            if let Some(distro) = s.strip_prefix("wsl:") {
                if !distro.is_empty() {
                    c.args(["-d", distro]);
                }
            }
            c
        }
        other => {
            let parts: Vec<&str> = other.splitn(2, ':').collect();
            let mut c = CommandBuilder::new(parts[0]);
            if let Some(args_str) = parts.get(1) {
                for arg in args_str.split(' ') {
                    if !arg.is_empty() {
                        c.arg(arg);
                    }
                }
            }
            c
        }
    };

    // portable_pty on Windows reads PATH from the Windows registry (HKLM + HKCU),
    // overwriting the inherited process PATH. The daemon prepends our binary directory
    // to PATH so the tmux shim is reachable, but portable_pty discards that modification.
    // Re-apply it here on the CommandBuilder so the shell always has the tmux shim in PATH.
    #[cfg(windows)]
    {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                if dir.join("tmux.exe").exists() {
                    if let Some(current_path) = cmd.get_env("PATH") {
                        let mut new_path = dir.as_os_str().to_os_string();
                        new_path.push(";");
                        new_path.push(current_path);
                        cmd.env("PATH", new_path);
                    } else {
                        cmd.env("PATH", dir.as_os_str());
                    }
                }
            }
        }
    }

    if let Some(dir) = cwd {
        cmd.cwd(dir);
    }
    if let Some(env_map) = env {
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }

    let child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|e| format!("spawn: {}", e))?;
    let shell_pid = child.process_id().unwrap_or(0);

    let writer = pair
        .master
        .take_writer()
        .map_err(|e| format!("take_writer: {}", e))?;
    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| format!("clone_reader: {}", e))?;

    drop(pair.slave);

    Ok(PtyParts {
        master: pair.master,
        writer,
        reader,
        shell_pid,
        child,
    })
}
