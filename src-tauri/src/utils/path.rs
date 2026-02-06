/// Convert Windows path to WSL format: C:\foo\bar -> /mnt/c/foo/bar
pub fn windows_to_wsl_path(windows_path: &str) -> String {
    // Handle empty path
    if windows_path.is_empty() {
        return String::new();
    }

    // Check if it's a Windows absolute path with drive letter (e.g., C:\)
    let chars: Vec<char> = windows_path.chars().collect();
    if chars.len() >= 2 && chars[0].is_ascii_alphabetic() && chars[1] == ':' {
        let drive_letter = chars[0].to_ascii_lowercase();
        let rest_of_path = &windows_path[2..];

        // Convert backslashes to forward slashes
        let unix_path = rest_of_path.replace('\\', "/");

        format!("/mnt/{}{}", drive_letter, unix_path)
    } else {
        // Not a Windows absolute path, just convert backslashes
        windows_path.replace('\\', "/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_to_wsl_path_basic() {
        assert_eq!(windows_to_wsl_path("C:\\Users\\foo"), "/mnt/c/Users/foo");
        assert_eq!(
            windows_to_wsl_path("D:\\Projects\\test"),
            "/mnt/d/Projects/test"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_empty() {
        assert_eq!(windows_to_wsl_path(""), "");
    }

    #[test]
    fn test_windows_to_wsl_path_relative() {
        assert_eq!(windows_to_wsl_path("relative\\path"), "relative/path");
    }

    #[test]
    fn test_windows_to_wsl_path_uppercase_drive() {
        assert_eq!(windows_to_wsl_path("C:\\foo"), "/mnt/c/foo");
        assert_eq!(windows_to_wsl_path("Z:\\bar"), "/mnt/z/bar");
    }

    #[test]
    fn test_windows_to_wsl_path_lowercase_drive() {
        assert_eq!(windows_to_wsl_path("c:\\foo"), "/mnt/c/foo");
        assert_eq!(windows_to_wsl_path("d:\\bar"), "/mnt/d/bar");
    }

    #[test]
    fn test_windows_to_wsl_path_trailing_slash() {
        assert_eq!(windows_to_wsl_path("C:\\"), "/mnt/c/");
        assert_eq!(windows_to_wsl_path("D:\\"), "/mnt/d/");
    }

    #[test]
    fn test_windows_to_wsl_path_root_only() {
        // Just the drive letter and colon
        assert_eq!(windows_to_wsl_path("C:"), "/mnt/c");
    }

    #[test]
    fn test_windows_to_wsl_path_deep_nesting() {
        assert_eq!(
            windows_to_wsl_path("C:\\Users\\alanm\\Documents\\dev\\project\\src\\main\\rust"),
            "/mnt/c/Users/alanm/Documents/dev/project/src/main/rust"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_with_spaces() {
        assert_eq!(
            windows_to_wsl_path("C:\\Program Files\\My App"),
            "/mnt/c/Program Files/My App"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_unc_path_no_panic() {
        // UNC paths should not panic, just convert backslashes
        let result = windows_to_wsl_path("\\\\server\\share\\folder");
        assert_eq!(result, "//server/share/folder");
    }

    #[test]
    fn test_windows_to_wsl_path_mixed_slashes() {
        // Mixed slashes (unusual but possible)
        assert_eq!(windows_to_wsl_path("C:\\foo/bar\\baz"), "/mnt/c/foo/bar/baz");
    }

    #[test]
    fn test_windows_to_wsl_path_forward_slashes_only() {
        // Already using forward slashes
        assert_eq!(windows_to_wsl_path("C:/Users/foo"), "/mnt/c/Users/foo");
    }
}
