use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use winapi::um::winbase::CREATE_NO_WINDOW;

/// Parse WSL distribution names from raw stdout bytes.
/// `wsl --list --quiet` outputs UTF-16LE on Windows, so we detect and decode it.
pub fn parse_wsl_distributions(stdout: &[u8]) -> Vec<String> {
    let decoded = decode_utf16le_or_utf8(stdout);

    decoded
        .lines()
        .map(|line| line.trim().trim_matches('\0').to_string())
        .filter(|line| !line.is_empty())
        .collect()
}

/// Decode bytes as UTF-16LE if they look like it, otherwise fall back to UTF-8.
fn decode_utf16le_or_utf8(bytes: &[u8]) -> String {
    let data = if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        &bytes[2..] // Strip UTF-16LE BOM
    } else {
        bytes
    };

    // Detect UTF-16LE: even length and every other byte (odd positions) is 0x00
    // for ASCII-range characters, which WSL distribution names always are.
    let looks_utf16le = data.len() >= 2
        && data.len() % 2 == 0
        && data.iter().skip(1).step_by(2).all(|&b| b == 0);

    if looks_utf16le {
        let u16_iter = data
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]));
        char::decode_utf16(u16_iter)
            .map(|r| r.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    } else {
        String::from_utf8_lossy(data).into_owned()
    }
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
        // This is detected as UTF-16LE and decoded properly
        let stdout = b"U\0b\0u\0n\0t\0u\0\n\0D\0e\0b\0i\0a\0n\0";
        let result = parse_wsl_distributions(stdout);
        assert_eq!(result, vec!["Ubuntu", "Debian"]);
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

    // Bug reproduction: WSL_E_DISTRO_NOT_FOUND when creating WSL workspace
    // `wsl --list --quiet` outputs UTF-16LE on Windows, but parse_wsl_distributions
    // uses from_utf8_lossy which mangles the names with embedded null chars.
    // When these mangled names are passed to `wsl.exe -d <name>`, WSL can't find them.

    #[test]
    fn test_parse_real_utf16le_wsl_output() {
        // Real output from `wsl --list --quiet` on Windows is UTF-16LE encoded.
        // "Ubuntu\r\n" in UTF-16LE:
        let stdout: Vec<u8> = vec![
            0x55, 0x00, // U
            0x62, 0x00, // b
            0x75, 0x00, // u
            0x6E, 0x00, // n
            0x74, 0x00, // t
            0x75, 0x00, // u
            0x0D, 0x00, // \r
            0x0A, 0x00, // \n
        ];

        let result = parse_wsl_distributions(&stdout);
        assert_eq!(
            result,
            vec!["Ubuntu"],
            "Bug: UTF-16LE output from wsl.exe is not decoded correctly, \
             producing mangled distribution names that cause WSL_E_DISTRO_NOT_FOUND"
        );
    }

    #[test]
    fn test_parse_real_utf16le_multiple_distros() {
        // Real output: "Ubuntu\r\ndocker-desktop\r\n" in UTF-16LE
        let stdout: Vec<u8> = "Ubuntu\r\ndocker-desktop\r\n"
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();

        let result = parse_wsl_distributions(&stdout);
        assert_eq!(
            result,
            vec!["Ubuntu", "docker-desktop"],
            "Bug: UTF-16LE output from wsl.exe is not decoded correctly, \
             producing mangled distribution names that cause WSL_E_DISTRO_NOT_FOUND"
        );
    }

    #[test]
    fn test_parse_utf16le_with_bom() {
        // Some Windows tools prepend a UTF-16LE BOM (0xFF 0xFE)
        let mut stdout: Vec<u8> = vec![0xFF, 0xFE]; // BOM
        stdout.extend(
            "Ubuntu\r\n"
                .encode_utf16()
                .flat_map(|c| c.to_le_bytes()),
        );

        let result = parse_wsl_distributions(&stdout);
        assert_eq!(
            result,
            vec!["Ubuntu"],
            "Bug: UTF-16LE output with BOM from wsl.exe is not decoded correctly"
        );
    }

    #[test]
    fn test_parsed_distro_names_contain_no_null_bytes() {
        // Real output: "Ubuntu\r\n" in UTF-16LE
        let stdout: Vec<u8> = "Ubuntu\r\n"
            .encode_utf16()
            .flat_map(|c| c.to_le_bytes())
            .collect();

        let result = parse_wsl_distributions(&stdout);
        for name in &result {
            assert!(
                !name.contains('\0'),
                "Bug: distribution name '{}' contains null bytes — \
                 wsl.exe will fail with WSL_E_DISTRO_NOT_FOUND",
                name.escape_debug()
            );
        }
        assert!(!result.is_empty(), "Should have parsed at least one distro");
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
