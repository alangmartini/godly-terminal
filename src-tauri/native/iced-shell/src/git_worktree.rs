//! Git worktree operations for Godly Terminal's worktree mode.
//!
//! All functions are synchronous and use `std::process::Command` to shell out to git.
//! They are intended to be called from background threads via `Task::perform`.

use std::path::Path;
use std::process::Command;

/// Check if a directory is inside a git repository.
pub fn is_git_repo(folder_path: &str) -> bool {
    Command::new("git")
        .args(["-C", folder_path, "rev-parse", "--is-inside-work-tree"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Find the root directory of the git repository containing `folder_path`.
pub fn find_repo_root(folder_path: &str) -> Result<String, String> {
    let output = Command::new("git")
        .args(["-C", folder_path, "rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Not a git repository: {stderr}"));
    }

    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if root.is_empty() {
        return Err("git rev-parse returned empty root".to_string());
    }
    Ok(root)
}

/// Generate a unique worktree directory name.
pub fn generate_worktree_dir_name() -> String {
    let id = uuid::Uuid::new_v4().to_string();
    format!("godly-wt-{}", &id[..8])
}

/// Resolve the base directory for worktrees.
///
/// Uses `%APPDATA%/com.godly.terminal/worktrees/` so that worktrees live
/// outside the source repository, avoiding the git-inside-git problem that
/// causes Claude agents to navigate back to the parent repo.
fn worktrees_base_dir() -> Result<std::path::PathBuf, String> {
    let appdata = std::env::var("APPDATA")
        .map_err(|_| "APPDATA environment variable not set".to_string())?;
    Ok(Path::new(&appdata)
        .join("com.godly.terminal")
        .join("worktrees"))
}

/// Create a git worktree in detached HEAD mode.
///
/// Creates the worktree at `%APPDATA%/com.godly.terminal/worktrees/<dir_name>/`.
/// This places worktrees outside the source repo to avoid the git-inside-git
/// problem that causes Claude agents to navigate back to the parent repo.
/// Returns the absolute path to the created worktree directory.
pub fn create_worktree(repo_root: &str, dir_name: &str) -> Result<String, String> {
    let worktrees_dir = worktrees_base_dir()?;
    let worktree_path = worktrees_dir.join(dir_name);
    let worktree_path_str = worktree_path
        .to_str()
        .ok_or_else(|| "Worktree path contains invalid characters".to_string())?
        .to_string();

    // Ensure parent directory exists.
    if !worktrees_dir.exists() {
        std::fs::create_dir_all(&worktrees_dir)
            .map_err(|e| format!("Failed to create worktrees directory: {e}"))?;
    }

    let output = Command::new("git")
        .args([
            "-C",
            repo_root,
            "worktree",
            "add",
            "--detach",
            &worktree_path_str,
        ])
        .output()
        .map_err(|e| format!("Failed to run git worktree add: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree add failed: {stderr}"));
    }

    Ok(worktree_path_str)
}

/// Remove a git worktree.
///
/// Attempts `git worktree remove --force`, falling back to manual directory
/// removal + `git worktree prune` on failure.
pub fn remove_worktree(repo_root: &str, worktree_path: &str) -> Result<(), String> {
    // Try git worktree remove --force first.
    let output = Command::new("git")
        .args(["-C", repo_root, "worktree", "remove", "--force", worktree_path])
        .output()
        .map_err(|e| format!("Failed to run git worktree remove: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    // Fallback: remove directory manually and prune.
    let path = Path::new(worktree_path);
    if path.exists() {
        std::fs::remove_dir_all(path)
            .map_err(|e| format!("Failed to remove worktree directory: {e}"))?;
    }

    let _ = Command::new("git")
        .args(["-C", repo_root, "worktree", "prune"])
        .output();

    Ok(())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_worktree_dir_name_format() {
        let name = generate_worktree_dir_name();
        assert!(name.starts_with("godly-wt-"));
        assert_eq!(name.len(), "godly-wt-".len() + 8);
    }

    #[test]
    fn generate_worktree_dir_name_unique() {
        let a = generate_worktree_dir_name();
        let b = generate_worktree_dir_name();
        assert_ne!(a, b);
    }

    #[test]
    fn is_git_repo_on_nonexistent_dir() {
        assert!(!is_git_repo("/nonexistent/path/that/does/not/exist"));
    }
}
