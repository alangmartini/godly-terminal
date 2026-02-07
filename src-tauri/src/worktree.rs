use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use winapi::um::winbase::CREATE_NO_WINDOW;

/// Prefix used for worktree branches created by Godly Terminal.
const BRANCH_PREFIX: &str = "wt-";

/// Subdirectory under TEMP for all godly worktrees.
const WORKTREES_DIR: &str = "godly-worktrees";

/// Result of creating a new worktree.
#[derive(Debug, Clone)]
pub struct WorktreeResult {
    pub path: String,
    pub branch: String,
}

/// Info about a single worktree returned by `list_worktrees`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    pub commit: String,
    pub is_main: bool,
}

fn git_cmd() -> Command {
    let mut cmd = Command::new("git");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Check if `path` is inside a git repository.
pub fn is_git_repo(path: &str) -> bool {
    let output = git_cmd()
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(path)
        .output();

    match output {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Return the root of the git repository that contains `path`.
pub fn get_repo_root(path: &str) -> Result<String, String> {
    let output = git_cmd()
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(path)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        return Err("Not a git repository".to_string());
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Compute a short (8-char) hex hash of a string (used to namespace temp dirs).
fn short_hash(input: &str) -> String {
    let mut hasher = DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())[..8].to_string()
}

/// Compute the worktree folder/branch name suffix from either a custom name or terminal id.
fn wt_name_from(custom_name: Option<&str>, terminal_id: &str) -> String {
    match custom_name {
        Some(name) if !name.is_empty() => format!("{}{}", BRANCH_PREFIX, name),
        _ => format!("{}{}", BRANCH_PREFIX, &terminal_id[..6.min(terminal_id.len())]),
    }
}

/// Build the worktree path: `%TEMP%/godly-worktrees/<repo-hash>/wt-<name>/`
pub fn worktree_path(repo_root: &str, terminal_id: &str, custom_name: Option<&str>) -> PathBuf {
    let temp = std::env::temp_dir();
    let repo_hash = short_hash(repo_root);
    let wt_name = wt_name_from(custom_name, terminal_id);
    temp.join(WORKTREES_DIR).join(&repo_hash).join(&wt_name)
}

/// Create a new worktree for the given terminal.
/// If `custom_name` is provided, it is used as the branch/folder suffix instead of the terminal id prefix.
/// Returns a `WorktreeResult` with the path and branch name.
pub fn create_worktree(repo_root: &str, terminal_id: &str, custom_name: Option<&str>) -> Result<WorktreeResult, String> {
    let wt_path = worktree_path(repo_root, terminal_id, custom_name);
    let branch_name = wt_name_from(custom_name, terminal_id);

    // Ensure parent directory exists
    if let Some(parent) = wt_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create worktree parent dir: {}", e))?;
    }

    let wt_path_str = wt_path.to_string_lossy().to_string();

    let output = git_cmd()
        .args(["worktree", "add", &wt_path_str, "-b", &branch_name])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Failed to run git worktree add: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree add failed: {}", stderr.trim()));
    }

    Ok(WorktreeResult { path: wt_path_str, branch: branch_name })
}

/// List all worktrees for the repo at `repo_root`.
pub fn list_worktrees(repo_root: &str) -> Result<Vec<WorktreeInfo>, String> {
    let output = git_cmd()
        .args(["worktree", "list", "--porcelain"])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Failed to run git worktree list: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree list failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_path = String::new();
    let mut current_commit = String::new();
    let mut current_branch = String::new();
    let mut is_first = true;

    for line in stdout.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            // If we have a previous worktree accumulated, push it
            if !current_path.is_empty() {
                worktrees.push(WorktreeInfo {
                    path: current_path.clone(),
                    branch: current_branch.clone(),
                    commit: current_commit.clone(),
                    is_main: is_first,
                });
                is_first = false;
            } else if !is_first {
                // Should not happen, but handle gracefully
            }
            current_path = path.to_string();
            current_commit = String::new();
            current_branch = String::new();
        } else if let Some(hash) = line.strip_prefix("HEAD ") {
            current_commit = hash.to_string();
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            // branch refs/heads/main -> main
            current_branch = branch_ref
                .strip_prefix("refs/heads/")
                .unwrap_or(branch_ref)
                .to_string();
        } else if line.trim().is_empty() && !current_path.is_empty() {
            worktrees.push(WorktreeInfo {
                path: current_path.clone(),
                branch: current_branch.clone(),
                commit: current_commit.clone(),
                is_main: is_first,
            });
            is_first = false;
            current_path = String::new();
            current_commit = String::new();
            current_branch = String::new();
        }
    }

    // Push last entry if not yet pushed
    if !current_path.is_empty() {
        worktrees.push(WorktreeInfo {
            path: current_path,
            branch: current_branch,
            commit: current_commit,
            is_main: is_first,
        });
    }

    Ok(worktrees)
}

/// Remove a single worktree by its path.
pub fn remove_worktree(repo_root: &str, wt_path: &str, force: bool) -> Result<(), String> {
    let mut args = vec!["worktree", "remove", wt_path];
    if force {
        args.push("--force");
    }

    let output = git_cmd()
        .args(&args)
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Failed to run git worktree remove: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree remove failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Remove all godly-managed worktrees (those in the temp directory) for a repo.
/// Returns the number of worktrees removed.
pub fn cleanup_all_worktrees(repo_root: &str) -> Result<u32, String> {
    let worktrees = list_worktrees(repo_root)?;
    let temp = std::env::temp_dir();
    let godly_prefix = temp.join(WORKTREES_DIR);
    let godly_prefix_str = godly_prefix.to_string_lossy().to_string();

    let mut removed = 0u32;
    for wt in &worktrees {
        if wt.is_main {
            continue;
        }
        // Only remove worktrees that live under our managed directory
        let normalized = wt.path.replace('/', "\\");
        let normalized_prefix = godly_prefix_str.replace('/', "\\");
        if normalized.starts_with(&normalized_prefix) || wt.path.starts_with(&godly_prefix_str) {
            match remove_worktree(repo_root, &wt.path, true) {
                Ok(()) => removed += 1,
                Err(e) => eprintln!("[worktree] Warning: failed to remove {}: {}", wt.path, e),
            }
        }
    }

    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command as StdCommand;

    /// Helper: create a throwaway git repo in a temp directory.
    fn create_test_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path();

        // git init
        let status = StdCommand::new("git")
            .args(["init"])
            .current_dir(path)
            .output()
            .expect("git init");
        assert!(status.status.success(), "git init failed");

        // Configure user for commits
        let _ = StdCommand::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(path)
            .output();
        let _ = StdCommand::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(path)
            .output();

        // Create an initial commit (required for worktrees)
        std::fs::write(path.join("README.md"), "# test").expect("write file");
        let _ = StdCommand::new("git")
            .args(["add", "."])
            .current_dir(path)
            .output();
        let _ = StdCommand::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(path)
            .output();

        dir
    }

    #[test]
    fn test_is_git_repo_positive() {
        let repo = create_test_repo();
        assert!(is_git_repo(repo.path().to_str().unwrap()));
    }

    #[test]
    fn test_is_git_repo_negative() {
        let dir = tempfile::tempdir().expect("create temp dir");
        assert!(!is_git_repo(dir.path().to_str().unwrap()));
    }

    #[test]
    fn test_get_repo_root() {
        let repo = create_test_repo();
        let root = get_repo_root(repo.path().to_str().unwrap()).expect("get repo root");
        // Normalize for comparison (Windows paths may differ in case/slashes)
        let expected = repo.path().canonicalize().unwrap();
        let got = Path::new(&root).canonicalize().unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn test_create_worktree() {
        let repo = create_test_repo();
        let repo_root = repo.path().to_str().unwrap();
        let terminal_id = "abcdef-1234-5678";

        let result = create_worktree(repo_root, terminal_id, None).expect("create worktree");
        assert!(Path::new(&result.path).is_dir(), "worktree directory should exist");
        assert_eq!(result.branch, "wt-abcdef");

        // Verify branch exists
        let output = StdCommand::new("git")
            .args(["branch", "--list", "wt-abcdef"])
            .current_dir(repo_root)
            .output()
            .expect("git branch list");
        let branches = String::from_utf8_lossy(&output.stdout);
        assert!(branches.contains("wt-abcdef"), "branch should exist");

        // Cleanup
        let _ = std::fs::remove_dir_all(&result.path);
    }

    #[test]
    fn test_create_worktree_custom_name() {
        let repo = create_test_repo();
        let repo_root = repo.path().to_str().unwrap();
        let terminal_id = "abcdef-1234-5678";

        let result = create_worktree(repo_root, terminal_id, Some("my-feature")).expect("create worktree with custom name");
        assert!(Path::new(&result.path).is_dir(), "worktree directory should exist");
        assert_eq!(result.branch, "wt-my-feature");

        // Verify branch exists
        let output = StdCommand::new("git")
            .args(["branch", "--list", "wt-my-feature"])
            .current_dir(repo_root)
            .output()
            .expect("git branch list");
        let branches = String::from_utf8_lossy(&output.stdout);
        assert!(branches.contains("wt-my-feature"), "custom branch should exist");

        // Cleanup
        let _ = std::fs::remove_dir_all(&result.path);
    }

    #[test]
    fn test_list_worktrees() {
        let repo = create_test_repo();
        let repo_root = repo.path().to_str().unwrap();

        // Initially just the main worktree
        let worktrees = list_worktrees(repo_root).expect("list worktrees");
        assert_eq!(worktrees.len(), 1, "should have 1 worktree (main)");
        assert!(worktrees[0].is_main);

        // Create a worktree
        let terminal_id = "112233-aabb-ccdd";
        let result = create_worktree(repo_root, terminal_id, None).expect("create worktree");

        let worktrees = list_worktrees(repo_root).expect("list worktrees after create");
        assert_eq!(worktrees.len(), 2, "should have 2 worktrees");

        // Cleanup
        let _ = std::fs::remove_dir_all(&result.path);
    }

    #[test]
    fn test_remove_worktree() {
        let repo = create_test_repo();
        let repo_root = repo.path().to_str().unwrap();
        let terminal_id = "aabbcc-1111-2222";

        let result = create_worktree(repo_root, terminal_id, None).expect("create worktree");
        assert!(Path::new(&result.path).is_dir());

        remove_worktree(repo_root, &result.path, false).expect("remove worktree");
        assert!(!Path::new(&result.path).is_dir(), "worktree should be removed");
    }

    #[test]
    fn test_create_worktree_non_git_fails() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let result = create_worktree(dir.path().to_str().unwrap(), "test-id", None);
        assert!(result.is_err(), "should fail on non-git dir");
    }

    #[test]
    fn test_short_hash_deterministic() {
        let h1 = short_hash("C:\\Users\\test\\repo");
        let h2 = short_hash("C:\\Users\\test\\repo");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 8);
    }

    #[test]
    fn test_worktree_path_formula() {
        let path = worktree_path("C:\\repo", "abcdef-1234", None);
        let path_str = path.to_string_lossy().to_string();
        assert!(path_str.contains(WORKTREES_DIR));
        assert!(path_str.contains("wt-abcdef"));
    }

    #[test]
    fn test_worktree_path_custom_name() {
        let path = worktree_path("C:\\repo", "abcdef-1234", Some("my-feature"));
        let path_str = path.to_string_lossy().to_string();
        assert!(path_str.contains(WORKTREES_DIR));
        assert!(path_str.contains("wt-my-feature"));
    }
}
