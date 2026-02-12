use std::io::Write;
use std::process::ExitCode;

use godly_protocol::{McpRequest, McpResponse};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("godly-notify: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let (terminal_id, message) = parse_args()?;
    let mut pipe = connect_pipe()?;
    send_notify(&mut pipe, terminal_id, message)
}

fn parse_args() -> Result<(String, Option<String>), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let mut terminal_id: Option<String> = None;
    let mut message: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--terminal-id" => {
                i += 1;
                terminal_id = Some(
                    args.get(i)
                        .ok_or("--terminal-id requires a value")?
                        .clone(),
                );
            }
            "--help" | "-h" => {
                eprintln!("Usage: godly-notify [--terminal-id <ID>] [MESSAGE]");
                eprintln!();
                eprintln!("Send a notification to Godly Terminal.");
                eprintln!();
                eprintln!("If --terminal-id is not provided, falls back to GODLY_SESSION_ID env var.");
                std::process::exit(0);
            }
            arg if arg.starts_with("--") => {
                return Err(format!("unknown flag: {arg}"));
            }
            _ => {
                message = Some(args[i..].join(" "));
                break;
            }
        }
        i += 1;
    }

    let terminal_id = terminal_id
        .or_else(|| std::env::var("GODLY_SESSION_ID").ok())
        .ok_or(
            "no terminal ID provided. Use --terminal-id <ID> or set GODLY_SESSION_ID env var",
        )?;

    Ok((terminal_id, message))
}

#[cfg(windows)]
fn connect_pipe() -> Result<std::fs::File, String> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    use std::os::windows::io::FromRawHandle;
    use winapi::um::errhandlingapi::GetLastError;
    use winapi::um::fileapi::{CreateFileW, OPEN_EXISTING};
    use winapi::um::handleapi::INVALID_HANDLE_VALUE;
    use winapi::um::winnt::{FILE_SHARE_READ, FILE_SHARE_WRITE, GENERIC_READ, GENERIC_WRITE};

    let pipe_name_str = godly_protocol::mcp_pipe_name();
    let pipe_name: Vec<u16> = OsStr::new(&pipe_name_str)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let handle = unsafe {
        CreateFileW(
            pipe_name.as_ptr(),
            GENERIC_READ | GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null_mut(),
            OPEN_EXISTING,
            0,
            std::ptr::null_mut(),
        )
    };

    if handle == INVALID_HANDLE_VALUE {
        let err = unsafe { GetLastError() };
        return Err(format!(
            "cannot connect to MCP pipe (error: {err}). Is Godly Terminal running?"
        ));
    }

    Ok(unsafe { std::fs::File::from_raw_handle(handle as _) })
}

#[cfg(not(windows))]
fn connect_pipe() -> Result<std::fs::File, String> {
    Err("godly-notify only supports Windows".to_string())
}

fn send_notify(
    pipe: &mut std::fs::File,
    terminal_id: String,
    message: Option<String>,
) -> Result<(), String> {
    let request = McpRequest::Notify {
        terminal_id,
        message,
    };

    godly_protocol::write_message(pipe, &request).map_err(|e| format!("write failed: {e}"))?;
    pipe.flush().ok();

    match godly_protocol::read_message::<_, McpResponse>(pipe) {
        Ok(Some(McpResponse::Ok)) => Ok(()),
        Ok(Some(McpResponse::Error { message })) => Err(format!("server error: {message}")),
        Ok(Some(other)) => Err(format!("unexpected response: {other:?}")),
        Ok(None) => Err("pipe closed before response".to_string()),
        Err(e) => Err(format!("read failed: {e}")),
    }
}
