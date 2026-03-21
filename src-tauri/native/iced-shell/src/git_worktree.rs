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

/// Create a git worktree in detached HEAD mode.
///
/// Creates the worktree at `<repo_root>/.godly-worktrees/<dir_name>/`.
/// Returns the absolute path to the created worktree directory.
pub fn create_worktree(repo_root: &str, dir_name: &str) -> Result<String, String> {
    let worktrees_dir = Path::new(repo_root).join(".godly-worktrees");
    let worktree_path = worktrees_dir.join(dir_name);
    let worktree_path_str = worktree_path
        .to_str()
        .ok_or_else(|| "Worktree path contains invalid characters".to_string())?
        .to_string();

    // Ensure parent directory exists.
    if !worktrees_dir.exists() {
        std::fs::create_dir_all(&worktrees_dir)
            .map_err(|e| format!("Failed to create .godly-worktrees directory: {e}"))?;
    }

    // Auto-add .godly-worktrees/ to .gitignore if not already present.
    ensure_gitignore(repo_root);

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

/// Ensure `.godly-worktrees/` is in the repo's `.gitignore`.
fn ensure_gitignore(repo_root: &str) {
    let gitignore_path = Path::new(repo_root).join(".gitignore");
    let entry = ".godly-worktrees/";

    if gitignore_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&gitignore_path) {
            if content.lines().any(|line| line.trim() == entry) {
                return; // Already present.
            }
            // Append to existing .gitignore.
            let separator = if content.ends_with('\n') { "" } else { "\n" };
            let new_content = format!("{content}{separator}{entry}\n");
            let _ = std::fs::write(&gitignore_path, new_content);
        }
    } else {
        // Create new .gitignore with just our entry.
        let _ = std::fs::write(&gitignore_path, format!("{entry}\n"));
    }
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
