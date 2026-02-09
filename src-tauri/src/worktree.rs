use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::{Command, Output};
use std::time::Instant;

use crate::utils::path::windows_to_wsl_path;

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

/// Progress information emitted during `cleanup_all_worktrees_with_progress`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CleanupProgress {
    pub step: String,
    pub current: u32,
    pub total: u32,
    pub worktree_name: String,
}

/// Info about a single worktree returned by `list_worktrees`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorktreeInfo {
    pub path: String,
    pub branch: String,
    pub commit: String,
    pub is_main: bool,
}

/// WSL configuration for running git commands inside a WSL distribution.
#[derive(Debug, Clone)]
pub struct WslConfig {
    pub distribution: Option<String>,
}

impl WslConfig {
    /// Auto-detect WSL configuration from a Windows UNC path.
    /// Returns `Some` if the path is a WSL UNC path (e.g. `\\wsl.localhost\Ubuntu\...`).
    pub fn from_path(path: &str) -> Option<Self> {
        let normalized = path.replace('\\', "/");
        let after_prefix = if normalized.starts_with("//wsl.localhost/") {
            Some(&normalized["//wsl.localhost/".len()..])
        } else if normalized.starts_with("//wsl$/") {
            Some(&normalized["//wsl$/".len()..])
        } else {
            None
        };

        after_prefix.map(|s| {
            let distribution = s.split('/').next()
                .filter(|d| !d.is_empty())
                .map(|d| d.to_string());
            WslConfig { distribution }
        })
    }
}

fn git_cmd() -> Command {
    let mut cmd = Command::new("git");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// Run a git command, optionally through WSL.
///
/// For WSL, the `cwd` is converted to a Linux path and passed via `wsl.exe --cd`.
/// The `cwd` can be either a Windows UNC path or an already-converted Linux path.
fn run_git(args: &[&str], cwd: &str, wsl: Option<&WslConfig>) -> std::io::Result<Output> {
    match wsl {
        Some(config) => {
            let linux_cwd = windows_to_wsl_path(cwd);
            let mut cmd = Command::new("wsl.exe");
            #[cfg(windows)]
            cmd.creation_flags(CREATE_NO_WINDOW);
            if let Some(distro) = &config.distribution {
                cmd.args(["-d", distro]);
            }
            cmd.args(["--cd", &linux_cwd, "--", "git"]);
            cmd.args(args);
            cmd.output()
        }
        None => {
            let mut cmd = git_cmd();
            cmd.args(args);
            cmd.current_dir(cwd);
            cmd.output()
        }
    }
}

/// Run a shell command inside WSL (e.g. `mkdir -p`).
fn run_wsl_cmd(config: &WslConfig, program: &str, args: &[&str]) -> std::io::Result<Output> {
    let mut cmd = Command::new("wsl.exe");
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    if let Some(distro) = &config.distribution {
        cmd.args(["-d", distro]);
    }
    cmd.args(["--", program]);
    cmd.args(args);
    cmd.output()
}

/// Check if `path` is inside a git repository.
pub fn is_git_repo(path: &str, wsl: Option<&WslConfig>) -> bool {
    match run_git(&["rev-parse", "--is-inside-work-tree"], path, wsl) {
        Ok(o) => o.status.success(),
        Err(_) => false,
    }
}

/// Return the root of the git repository that contains `path`.
pub fn get_repo_root(path: &str, wsl: Option<&WslConfig>) -> Result<String, String> {
    let output = run_git(&["rev-parse", "--show-toplevel"], path, wsl)
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

/// Build the worktree path for WSL: `/tmp/godly-worktrees/<repo-hash>/wt-<name>`
fn worktree_path_wsl(repo_root: &str, terminal_id: &str, custom_name: Option<&str>) -> String {
    let repo_hash = short_hash(repo_root);
    let wt_name = wt_name_from(custom_name, terminal_id);
    format!("/tmp/{}/{}/{}", WORKTREES_DIR, repo_hash, wt_name)
}

/// Create a new worktree for the given terminal.
/// If `custom_name` is provided, it is used as the branch/folder suffix instead of the terminal id prefix.
/// Returns a `WorktreeResult` with the path and branch name.
pub fn create_worktree(repo_root: &str, terminal_id: &str, custom_name: Option<&str>, wsl: Option<&WslConfig>) -> Result<WorktreeResult, String> {
    let branch_name = wt_name_from(custom_name, terminal_id);

    if let Some(wsl_config) = wsl {
        // WSL: create worktree inside WSL's /tmp
        let wt_linux_path = worktree_path_wsl(repo_root, terminal_id, custom_name);

        // Ensure parent directory exists inside WSL
        if let Some(parent) = wt_linux_path.rsplit_once('/').map(|(p, _)| p) {
            let _ = run_wsl_cmd(wsl_config, "mkdir", &["-p", parent]);
        }

        let output = run_git(
            &["worktree", "add", &wt_linux_path, "-b", &branch_name],
            repo_root, Some(wsl_config),
        )
        .map_err(|e| format!("Failed to run git worktree add: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git worktree add failed: {}", stderr.trim()));
        }

        // Return the Linux path — the daemon's windows_to_wsl_path passes it through unchanged
        Ok(WorktreeResult { path: wt_linux_path, branch: branch_name })
    } else {
        // Windows: create worktree in %TEMP%
        let wt_path = worktree_path(repo_root, terminal_id, custom_name);

        if let Some(parent) = wt_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create worktree parent dir: {}", e))?;
        }

        let wt_path_str = wt_path.to_string_lossy().to_string();

        let output = run_git(
            &["worktree", "add", &wt_path_str, "-b", &branch_name],
            repo_root, None,
        )
        .map_err(|e| format!("Failed to run git worktree add: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("git worktree add failed: {}", stderr.trim()));
        }

        Ok(WorktreeResult { path: wt_path_str, branch: branch_name })
    }
}

/// List all worktrees for the repo at `repo_root`.
pub fn list_worktrees(repo_root: &str, wsl: Option<&WslConfig>) -> Result<Vec<WorktreeInfo>, String> {
    let output = run_git(&["worktree", "list", "--porcelain"], repo_root, wsl)
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
pub fn remove_worktree(repo_root: &str, wt_path: &str, force: bool, wsl: Option<&WslConfig>) -> Result<(), String> {
    let mut args = vec!["worktree", "remove", wt_path];
    if force {
        args.push("--force");
    }

    let output = run_git(&args, repo_root, wsl)
        .map_err(|e| format!("Failed to run git worktree remove: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree remove failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Remove all godly-managed worktrees with progress reporting.
/// Calls `on_progress` at each stage: "listing", "removing", and "done".
/// Returns the number of worktrees removed.
pub fn cleanup_all_worktrees_with_progress<F>(repo_root: &str, on_progress: F, wsl: Option<&WslConfig>) -> Result<u32, String>
where
    F: Fn(&CleanupProgress),
{
    let start = Instant::now();
    eprintln!("[worktree] cleanup_all: listing worktrees...");

    on_progress(&CleanupProgress {
        step: "listing".to_string(),
        current: 0,
        total: 0,
        worktree_name: String::new(),
    });

    let worktrees = list_worktrees(repo_root, wsl)?;

    // Filter to godly-managed worktrees
    let managed: Vec<&WorktreeInfo> = if wsl.is_some() {
        // WSL: worktrees are in /tmp/godly-worktrees/
        let godly_prefix = format!("/tmp/{}", WORKTREES_DIR);
        worktrees
            .iter()
            .filter(|wt| !wt.is_main && wt.path.starts_with(&godly_prefix))
            .collect()
    } else {
        // Windows: worktrees are in %TEMP%/godly-worktrees/
        let temp = std::env::temp_dir();
        let godly_prefix = temp.join(WORKTREES_DIR);
        let godly_prefix_str = godly_prefix.to_string_lossy().to_string();
        worktrees
            .iter()
            .filter(|wt| {
                if wt.is_main {
                    return false;
                }
                let normalized = wt.path.replace('/', "\\");
                let normalized_prefix = godly_prefix_str.replace('/', "\\");
                normalized.starts_with(&normalized_prefix) || wt.path.starts_with(&godly_prefix_str)
            })
            .collect()
    };

    let total = managed.len() as u32;
    eprintln!("[worktree] cleanup_all: found {} godly-managed worktrees", total);

    let mut removed = 0u32;
    for (i, wt) in managed.iter().enumerate() {
        let name = wt.branch.clone();
        eprintln!("[worktree] cleanup_all: removing {} ({}/{})", name, i + 1, total);

        on_progress(&CleanupProgress {
            step: "removing".to_string(),
            current: i as u32 + 1,
            total,
            worktree_name: name.clone(),
        });

        match remove_worktree(repo_root, &wt.path, true, wsl) {
            Ok(()) => removed += 1,
            Err(e) => eprintln!("[worktree] Warning: failed to remove {}: {}", wt.path, e),
        }
    }

    let elapsed = start.elapsed();
    eprintln!("[worktree] cleanup_all: done — removed {} worktrees in {:.1}s", removed, elapsed.as_secs_f64());

    on_progress(&CleanupProgress {
        step: "done".to_string(),
        current: removed,
        total,
        worktree_name: String::new(),
    });

    Ok(removed)
}

/// Remove all godly-managed worktrees (those in the temp directory) for a repo.
/// Returns the number of worktrees removed.
pub fn cleanup_all_worktrees(repo_root: &str, wsl: Option<&WslConfig>) -> Result<u32, String> {
    cleanup_all_worktrees_with_progress(repo_root, |_| {}, wsl)
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
        assert!(is_git_repo(repo.path().to_str().unwrap(), None));
    }

    #[test]
    fn test_is_git_repo_negative() {
        let dir = tempfile::tempdir().expect("create temp dir");
        assert!(!is_git_repo(dir.path().to_str().unwrap(), None));
    }

    #[test]
    fn test_get_repo_root() {
        let repo = create_test_repo();
        let root = get_repo_root(repo.path().to_str().unwrap(), None).expect("get repo root");
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

        let result = create_worktree(repo_root, terminal_id, None, None).expect("create worktree");
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

        let result = create_worktree(repo_root, terminal_id, Some("my-feature"), None).expect("create worktree with custom name");
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
        let worktrees = list_worktrees(repo_root, None).expect("list worktrees");
        assert_eq!(worktrees.len(), 1, "should have 1 worktree (main)");
        assert!(worktrees[0].is_main);

        // Create a worktree
        let terminal_id = "112233-aabb-ccdd";
        let result = create_worktree(repo_root, terminal_id, None, None).expect("create worktree");

        let worktrees = list_worktrees(repo_root, None).expect("list worktrees after create");
        assert_eq!(worktrees.len(), 2, "should have 2 worktrees");

        // Cleanup
        let _ = std::fs::remove_dir_all(&result.path);
    }

    #[test]
    fn test_remove_worktree() {
        let repo = create_test_repo();
        let repo_root = repo.path().to_str().unwrap();
        let terminal_id = "aabbcc-1111-2222";

        let result = create_worktree(repo_root, terminal_id, None, None).expect("create worktree");
        assert!(Path::new(&result.path).is_dir());

        remove_worktree(repo_root, &result.path, false, None).expect("remove worktree");
        assert!(!Path::new(&result.path).is_dir(), "worktree should be removed");
    }

    #[test]
    fn test_create_worktree_non_git_fails() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let result = create_worktree(dir.path().to_str().unwrap(), "test-id", None, None);
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
        let expected = std::env::temp_dir()
            .join(WORKTREES_DIR)
            .join(short_hash("C:\\repo"))
            .join("wt-abcdef");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_worktree_path_custom_name() {
        let path = worktree_path("C:\\repo", "abcdef-1234", Some("my-feature"));
        let expected = std::env::temp_dir()
            .join(WORKTREES_DIR)
            .join(short_hash("C:\\repo"))
            .join("wt-my-feature");
        assert_eq!(path, expected);
    }

    #[test]
    fn test_cleanup_all_worktrees_with_progress() {
        use std::sync::{Arc, Mutex};

        let repo = create_test_repo();
        let repo_root = repo.path().to_str().unwrap();

        // Create 2 worktrees (IDs must have different first-6-char prefixes)
        let wt1 = create_worktree(repo_root, "aaaaaa-prog-test-1", None, None)
            .expect("create worktree 1");
        let wt2 = create_worktree(repo_root, "bbbbbb-prog-test-2", None, None)
            .expect("create worktree 2");

        assert!(Path::new(&wt1.path).is_dir());
        assert!(Path::new(&wt2.path).is_dir());

        // Collect progress events
        let events: Arc<Mutex<Vec<CleanupProgress>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();

        let removed = cleanup_all_worktrees_with_progress(repo_root, move |progress| {
            events_clone.lock().unwrap().push(progress.clone());
        }, None)
        .expect("cleanup should succeed");

        assert_eq!(removed, 2, "should have removed 2 worktrees");

        // Verify worktree directories were actually removed
        assert!(!Path::new(&wt1.path).is_dir(), "worktree 1 directory should be deleted");
        assert!(!Path::new(&wt2.path).is_dir(), "worktree 2 directory should be deleted");

        let collected = events.lock().unwrap();
        // Exactly 4 events: listing, removing x2, done
        assert_eq!(collected.len(), 4, "expected exactly 4 progress events, got {}", collected.len());

        // Event 0: listing
        assert_eq!(collected[0].step, "listing");
        assert_eq!(collected[0].current, 0);
        assert_eq!(collected[0].total, 0);

        // Events 1-2: removing (with correct current/total and non-empty name)
        assert_eq!(collected[1].step, "removing");
        assert_eq!(collected[1].current, 1);
        assert_eq!(collected[1].total, 2);
        assert!(!collected[1].worktree_name.is_empty(), "worktree_name should not be empty");

        assert_eq!(collected[2].step, "removing");
        assert_eq!(collected[2].current, 2);
        assert_eq!(collected[2].total, 2);
        assert!(!collected[2].worktree_name.is_empty(), "worktree_name should not be empty");

        // Event 3: done
        assert_eq!(collected[3].step, "done");
        assert_eq!(collected[3].current, 2); // current = removed count
        assert_eq!(collected[3].total, 2);
    }

    // --- WslConfig auto-detection tests ---

    #[test]
    fn test_wsl_config_from_wsl_localhost_path() {
        let config = WslConfig::from_path("\\\\wsl.localhost\\Ubuntu\\home\\user\\project");
        assert!(config.is_some());
        assert_eq!(config.unwrap().distribution.as_deref(), Some("Ubuntu"));
    }

    #[test]
    fn test_wsl_config_from_wsl_dollar_path() {
        let config = WslConfig::from_path("\\\\wsl$\\Debian\\home\\user\\project");
        assert!(config.is_some());
        assert_eq!(config.unwrap().distribution.as_deref(), Some("Debian"));
    }

    #[test]
    fn test_wsl_config_from_windows_path() {
        let config = WslConfig::from_path("C:\\Users\\user\\project");
        assert!(config.is_none());
    }

    #[test]
    fn test_wsl_config_from_linux_path() {
        let config = WslConfig::from_path("/home/user/project");
        assert!(config.is_none());
    }

    #[test]
    fn test_worktree_path_wsl() {
        let path = worktree_path_wsl("/home/user/project", "abcdef-1234", None);
        let expected = format!("/tmp/{}/{}/wt-abcdef", WORKTREES_DIR, short_hash("/home/user/project"));
        assert_eq!(path, expected);
    }

    #[test]
    fn test_worktree_path_wsl_custom_name() {
        let path = worktree_path_wsl("/home/user/project", "abcdef-1234", Some("my-feature"));
        let expected = format!("/tmp/{}/{}/wt-my-feature", WORKTREES_DIR, short_hash("/home/user/project"));
        assert_eq!(path, expected);
    }

    #[test]
    fn test_worktree_path_wsl_different_repos_different_paths() {
        let path_a = worktree_path_wsl("/home/user/repo-a", "abcdef-1234", None);
        let path_b = worktree_path_wsl("/home/user/repo-b", "abcdef-1234", None);
        assert_ne!(path_a, path_b, "different repos must produce different worktree paths");
    }

    #[test]
    fn test_wsl_config_from_forward_slash_unc() {
        // Pre-normalized UNC paths with forward slashes
        let config = WslConfig::from_path("//wsl.localhost/Ubuntu/home/user/project");
        assert!(config.is_some());
        assert_eq!(config.unwrap().distribution.as_deref(), Some("Ubuntu"));

        let config = WslConfig::from_path("//wsl$/Debian/home/user/project");
        assert!(config.is_some());
        assert_eq!(config.unwrap().distribution.as_deref(), Some("Debian"));
    }

    #[test]
    fn test_wsl_config_from_distro_only_path() {
        // UNC path pointing to just the distro root (no trailing path)
        let config = WslConfig::from_path("\\\\wsl.localhost\\Ubuntu");
        assert!(config.is_some());
        assert_eq!(config.unwrap().distribution.as_deref(), Some("Ubuntu"));
    }

    #[test]
    fn test_wt_name_from_empty_custom_name_falls_through() {
        // Empty custom name should fall back to terminal ID prefix
        let name = wt_name_from(Some(""), "abcdef-1234");
        assert_eq!(name, "wt-abcdef");
    }
}
