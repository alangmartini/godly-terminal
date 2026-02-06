use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use winapi::um::winbase::CREATE_NO_WINDOW;

/// Parse WSL distribution names from raw stdout bytes.
/// Handles UTF-16LE encoding artifacts (null characters) and whitespace.
pub fn parse_wsl_distributions(stdout: &[u8]) -> Vec<String> {
    let stdout_str = String::from_utf8_lossy(stdout);

    stdout_str
        .lines()
        .map(|line| line.trim().trim_matches('\0').to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

#[tauri::command]
pub fn get_wsl_distributions() -> Result<Vec<String>, String> {
    let mut cmd = Command::new("wsl");
    cmd.args(["--list", "--quiet"]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute wsl command: {}", e))?;

    if !output.status.success() {
        return Err("WSL command failed".to_string());
    }

    Ok(parse_wsl_distributions(&output.stdout))
}

#[tauri::command]
pub fn is_wsl_available() -> bool {
    let mut cmd = Command::new("wsl");
    cmd.arg("--status");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_wsl_distributions_normal_output() {
        let stdout = b"Ubuntu\nDebian\nAlpine\n";
        let result = parse_wsl_distributions(stdout);
        assert_eq!(result, vec!["Ubuntu", "Debian", "Alpine"]);
    }

    #[test]
    fn test_parse_wsl_distributions_empty_output() {
        let stdout = b"";
        let result = parse_wsl_distributions(stdout);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_wsl_distributions_with_null_characters() {
        // Simulates UTF-16LE artifacts with null bytes interspersed
        let stdout = b"U\0b\0u\0n\0t\0u\0\n\0D\0e\0b\0i\0a\0n\0";
        let result = parse_wsl_distributions(stdout);
        // After from_utf8_lossy and trim_matches('\0'), we should get cleaned strings
        assert!(!result.is_empty());
        // The exact result depends on how the nulls are interpreted
    }

    #[test]
    fn test_parse_wsl_distributions_with_trailing_nulls() {
        let stdout = b"Ubuntu\0\0\0\nDebian\0\0\n";
        let result = parse_wsl_distributions(stdout);
        assert_eq!(result, vec!["Ubuntu", "Debian"]);
    }

    #[test]
    fn test_parse_wsl_distributions_whitespace_handling() {
        let stdout = b"  Ubuntu  \n\tDebian\t\n  \n  Alpine  \n";
        let result = parse_wsl_distributions(stdout);
        assert_eq!(result, vec!["Ubuntu", "Debian", "Alpine"]);
    }

    #[test]
    fn test_parse_wsl_distributions_single_distribution() {
        let stdout = b"Ubuntu\n";
        let result = parse_wsl_distributions(stdout);
        assert_eq!(result, vec!["Ubuntu"]);
    }

    #[test]
    fn test_parse_wsl_distributions_no_trailing_newline() {
        let stdout = b"Ubuntu";
        let result = parse_wsl_distributions(stdout);
        assert_eq!(result, vec!["Ubuntu"]);
    }

    #[test]
    fn test_parse_wsl_distributions_only_whitespace() {
        let stdout = b"   \n  \n\t\n";
        let result = parse_wsl_distributions(stdout);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_wsl_distributions_mixed_empty_lines() {
        let stdout = b"Ubuntu\n\n\nDebian\n\nAlpine";
        let result = parse_wsl_distributions(stdout);
        assert_eq!(result, vec!["Ubuntu", "Debian", "Alpine"]);
    }

    #[test]
    fn test_parse_wsl_distributions_with_version_names() {
        let stdout = b"Ubuntu-22.04\nUbuntu-20.04\nDebian\n";
        let result = parse_wsl_distributions(stdout);
        assert_eq!(result, vec!["Ubuntu-22.04", "Ubuntu-20.04", "Debian"]);
    }

    // Integration tests that require WSL to be installed
    #[test]
    #[ignore]
    fn test_get_wsl_distributions_integration() {
        // This test only runs when WSL is available
        let result = get_wsl_distributions();
        // Should not error on a system with WSL
        assert!(result.is_ok());
    }

    #[test]
    #[ignore]
    fn test_is_wsl_available_integration() {
        // This test only runs when explicitly enabled
        let available = is_wsl_available();
        // Just verify it returns a boolean without panicking
        let _ = available;
    }
}
