/// Convert Windows path to WSL format: C:\foo\bar -> /mnt/c/foo/bar
#[allow(dead_code)]
pub fn windows_to_wsl_path(windows_path: &str) -> String {
    // Handle empty path
    if windows_path.is_empty() {
        return String::new();
    }

    // Normalize backslashes first
    let path = windows_path.replace('\\', "/");

    // Handle WSL UNC paths: //wsl.localhost/<distro>/... or //wsl$/<distro>/...
    // These must be converted to native Linux paths by stripping the prefix and distro name.
    if path.starts_with("//wsl.localhost/") || path.starts_with("//wsl$/") {
        let after_host = if path.starts_with("//wsl.localhost/") {
            &path["//wsl.localhost/".len()..]
        } else {
            &path["//wsl$/".len()..]
        };
        // Skip the distro name (first path segment)
        return match after_host.find('/') {
            Some(idx) => {
                let linux_path = &after_host[idx..];
                if linux_path == "/" { "/".to_string() } else { linux_path.to_string() }
            }
            None => "/".to_string(),
        };
    }

    // Check if it's a Windows absolute path with drive letter (e.g., C:\)
    let chars: Vec<char> = path.chars().collect();
    if chars.len() >= 2 && chars[0].is_ascii_alphabetic() && chars[1] == ':' {
        let drive_letter = chars[0].to_ascii_lowercase();
        let rest_of_path = &path[2..];
        format!("/mnt/{}{}", drive_letter, rest_of_path)
    } else {
        path
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

    // Bug: WSL UNC paths like \\wsl.localhost\Ubuntu\home\user\project are converted to
    // //wsl.localhost/Ubuntu/home/user/project instead of /home/user/project, causing
    // wsl.exe --cd to receive an invalid path and chdir() fails with error 2.
    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_unc() {
        // \\wsl.localhost\<distro>\<path> should strip the UNC prefix and distro name
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\home\\alanm\\dev\\project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_unc_forward_slashes() {
        // Same path with forward slashes (may arrive pre-normalized)
        assert_eq!(
            windows_to_wsl_path("//wsl.localhost/Ubuntu/home/alanm/dev/project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_dollar_unc() {
        // Legacy \\wsl$\<distro>\<path> format
        assert_eq!(
            windows_to_wsl_path("\\\\wsl$\\Ubuntu\\home\\alanm\\dev\\project"),
            "/home/alanm/dev/project"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_root() {
        // UNC path to distro root
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu"),
            "/"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_root_trailing_slash() {
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\"),
            "/"
        );
    }

    #[test]
    fn test_windows_to_wsl_path_wsl_localhost_deep_path() {
        // Exact path from bug report
        assert_eq!(
            windows_to_wsl_path("\\\\wsl.localhost\\Ubuntu\\home\\alanm\\dev\\terraform-tests\\terraform-provider-typesense"),
            "/home/alanm/dev/terraform-tests/terraform-provider-typesense"
        );
    }
}
