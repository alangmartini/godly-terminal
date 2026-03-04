use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Output};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use winapi::um::winbase::CREATE_NO_WINDOW;

use crate::state::{GitHubTokenRule, Workspace, WorkspaceGitHubAuthPolicy};
use crate::worktree::{self, WslConfig};

const ENV_GH_TOKEN: &str = "GH_TOKEN";
const ENV_GITHUB_TOKEN: &str = "GITHUB_TOKEN";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct MatchContext {
    repo_full: Option<String>,
    repo_slug: Option<String>,
    paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RepoIdentity {
    host: String,
    owner: String,
    repo: String,
}

impl RepoIdentity {
    fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.repo)
    }

    fn full(&self) -> String {
        format!("{}/{}", self.host, self.slug())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct RuleScore {
    tier: u8,
    specificity: usize,
    pattern_len: usize,
}

pub fn inject_workspace_github_token_env(
    env_vars: &mut HashMap<String, String>,
    workspace: &Workspace,
    working_dir: Option<&str>,
    policy: &WorkspaceGitHubAuthPolicy,
) {
    if policy.rules.is_empty() && !policy.fallback_to_gh_auth {
        return;
    }

    let match_ctx = build_match_context(workspace, working_dir);

    if let Some(rule) = select_best_rule(&policy.rules, &match_ctx) {
        if let Some(token) = load_token_from_env_var(&rule.token_env_var) {
            set_github_token_env(env_vars, &token);
            eprintln!(
                "[github_auth] Applied GitHub token rule '{}' for workspace {}",
                rule.pattern, workspace.id
            );
            return;
        }

        eprintln!(
            "[github_auth] Rule '{}' matched but env var '{}' is missing/empty; continuing to fallback",
            rule.pattern, rule.token_env_var
        );
    }

    if policy.fallback_to_gh_auth {
        if let Some(token) = resolve_gh_auth_token(workspace, working_dir) {
            set_github_token_env(env_vars, &token);
            eprintln!(
                "[github_auth] Applied GitHub token from `gh auth token` for workspace {}",
                workspace.id
            );
        }
    }
}

fn set_github_token_env(env_vars: &mut HashMap<String, String>, token: &str) {
    env_vars.insert(ENV_GH_TOKEN.to_string(), token.to_string());
    env_vars.insert(ENV_GITHUB_TOKEN.to_string(), token.to_string());
}

fn load_token_from_env_var(env_var: &str) -> Option<String> {
    let value = std::env::var(env_var).ok()?;
    let token = value.trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn resolve_gh_auth_token(workspace: &Workspace, working_dir: Option<&str>) -> Option<String> {
    let workspace_wsl = WslConfig::from_path(&workspace.folder_path);

    // Prefer running `gh` inside WSL for WSL workspaces.
    if let Some(wsl) = workspace_wsl.as_ref() {
        if let Some(token) = run_gh_auth_token_wsl(wsl) {
            return Some(token);
        }
    }

    // Try native `gh`, using cwd only if it's a valid local path.
    let preferred_cwd = working_dir.or(Some(workspace.folder_path.as_str()));
    if let Some(token) = run_gh_auth_token_native(preferred_cwd) {
        return Some(token);
    }

    // Last attempt with no cwd.
    run_gh_auth_token_native(None)
}

fn run_gh_auth_token_native(cwd: Option<&str>) -> Option<String> {
    let mut cmd = new_command("gh");
    cmd.args(["auth", "token"]);
    if let Some(dir) = cwd {
        let dir_path = Path::new(dir);
        if dir_path.is_dir() {
            cmd.current_dir(dir_path);
        }
    }
    parse_token_output(cmd.output().ok()?)
}

fn run_gh_auth_token_wsl(wsl: &WslConfig) -> Option<String> {
    let mut cmd = new_command("wsl.exe");
    if let Some(distro) = &wsl.distribution {
        cmd.args(["-d", distro]);
    }
    cmd.args(["--", "gh", "auth", "token"]);
    parse_token_output(cmd.output().ok()?)
}

fn parse_token_output(output: Output) -> Option<String> {
    if !output.status.success() {
        return None;
    }

    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn build_match_context(workspace: &Workspace, working_dir: Option<&str>) -> MatchContext {
    let repo_identity = resolve_repo_identity(workspace, working_dir);

    let mut paths = Vec::new();
    if let Some(wd) = working_dir {
        paths.push(normalize_for_match(wd));
    }
    let workspace_path = normalize_for_match(&workspace.folder_path);
    if !paths.iter().any(|p| p == &workspace_path) {
        paths.push(workspace_path);
    }

    MatchContext {
        repo_full: repo_identity.as_ref().map(RepoIdentity::full),
        repo_slug: repo_identity.as_ref().map(RepoIdentity::slug),
        paths,
    }
}

fn resolve_repo_identity(workspace: &Workspace, working_dir: Option<&str>) -> Option<RepoIdentity> {
    let wsl = WslConfig::from_path(&workspace.folder_path);

    if let Some(wd) = working_dir {
        if let Some(identity) = get_repo_identity_from_path(wd, wsl.as_ref()) {
            return Some(identity);
        }
    }

    get_repo_identity_from_path(&workspace.folder_path, wsl.as_ref())
}

fn get_repo_identity_from_path(path: &str, wsl: Option<&WslConfig>) -> Option<RepoIdentity> {
    let origin = worktree::get_origin_url(path, wsl).ok().flatten()?;
    parse_repo_identity(&origin)
}

fn parse_repo_identity(origin: &str) -> Option<RepoIdentity> {
    let trimmed = origin.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (host, path) = if let Some((_, rest)) = trimmed.split_once("://") {
        let (host_part, path_part) = rest.split_once('/')?;
        let host = host_part.rsplit('@').next()?.split(':').next()?.to_lowercase();
        (host, path_part.to_string())
    } else if let Some((left, right)) = trimmed.split_once(':') {
        // SCP-like syntax, e.g. git@github.com:owner/repo.git
        if !left.contains('@') && !left.contains('.') {
            return None;
        }
        let host = left.rsplit('@').next()?.to_lowercase();
        (host, right.to_string())
    } else {
        return None;
    };

    let path = path.trim_start_matches('/').trim_end_matches('/');
    let mut segments = path.split('/').filter(|segment| !segment.is_empty());
    let owner = segments.next()?.to_lowercase();
    let repo_raw = segments.next()?.to_lowercase();
    let repo = repo_raw.strip_suffix(".git").unwrap_or(&repo_raw).to_string();
    if repo.is_empty() {
        return None;
    }

    Some(RepoIdentity { host, owner, repo })
}

fn select_best_rule<'a>(
    rules: &'a [GitHubTokenRule],
    match_ctx: &MatchContext,
) -> Option<&'a GitHubTokenRule> {
    let mut best: Option<(usize, RuleScore)> = None;

    for (index, rule) in rules.iter().enumerate() {
        let score = match rule_score(rule, match_ctx) {
            Some(score) => score,
            None => continue,
        };

        match best {
            None => best = Some((index, score)),
            Some((best_index, best_score)) => {
                if score > best_score || (score == best_score && index < best_index) {
                    best = Some((index, score));
                }
            }
        }
    }

    best.map(|(index, _)| &rules[index])
}

fn rule_score(rule: &GitHubTokenRule, match_ctx: &MatchContext) -> Option<RuleScore> {
    let pattern = normalize_for_match(&rule.pattern);
    if pattern.is_empty() {
        return None;
    }

    let has_wildcard = pattern.contains('*') || pattern.contains('?');
    let specificity = pattern_specificity(&pattern);
    let pattern_len = pattern.len();

    if let Some(repo_slug) = match_ctx.repo_slug.as_deref() {
        let repo_full_match = match_ctx
            .repo_full
            .as_deref()
            .map(|repo_full| glob_matches(&pattern, repo_full))
            .unwrap_or(false);
        let repo_slug_match = glob_matches(&pattern, repo_slug);

        if repo_full_match || repo_slug_match {
            let is_exact_repo = !has_wildcard
                && (match_ctx.repo_full.as_deref() == Some(pattern.as_str())
                    || Some(repo_slug) == Some(pattern.as_str()));
            return Some(RuleScore {
                tier: if is_exact_repo { 3 } else { 2 },
                specificity,
                pattern_len,
            });
        }
    }

    if match_ctx
        .paths
        .iter()
        .any(|path| glob_matches(&pattern, path))
    {
        return Some(RuleScore {
            tier: 1,
            specificity,
            pattern_len,
        });
    }

    None
}

fn pattern_specificity(pattern: &str) -> usize {
    pattern
        .chars()
        .filter(|c| *c != '*' && *c != '?')
        .count()
}

fn normalize_for_match(input: &str) -> String {
    input
        .trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_lowercase()
}

fn glob_matches(pattern: &str, text: &str) -> bool {
    let pattern_bytes = pattern.as_bytes();
    let text_bytes = text.as_bytes();

    let mut pattern_index = 0usize;
    let mut text_index = 0usize;
    let mut star_index: Option<usize> = None;
    let mut text_checkpoint = 0usize;

    while text_index < text_bytes.len() {
        if pattern_index < pattern_bytes.len()
            && (pattern_bytes[pattern_index] == b'?'
                || pattern_bytes[pattern_index] == text_bytes[text_index])
        {
            pattern_index += 1;
            text_index += 1;
            continue;
        }

        if pattern_index < pattern_bytes.len() && pattern_bytes[pattern_index] == b'*' {
            star_index = Some(pattern_index);
            pattern_index += 1;
            text_checkpoint = text_index;
            continue;
        }

        if let Some(star) = star_index {
            pattern_index = star + 1;
            text_checkpoint += 1;
            text_index = text_checkpoint;
            continue;
        }

        return false;
    }

    while pattern_index < pattern_bytes.len() && pattern_bytes[pattern_index] == b'*' {
        pattern_index += 1;
    }

    pattern_index == pattern_bytes.len()
}

fn new_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_matches_basic_patterns() {
        assert!(glob_matches("typesense/*", "typesense/typesense"));
        assert!(glob_matches("*", "typesense/typesense"));
        assert!(glob_matches(
            "github.com/typesense/*",
            "github.com/typesense/typesense"
        ));
        assert!(!glob_matches("typesense/*", "other-org/repo"));
    }

    #[test]
    fn parse_repo_identity_from_https_origin() {
        let identity = parse_repo_identity("https://github.com/typesense/typesense.git").unwrap();
        assert_eq!(identity.host, "github.com");
        assert_eq!(identity.owner, "typesense");
        assert_eq!(identity.repo, "typesense");
        assert_eq!(identity.slug(), "typesense/typesense");
    }

    #[test]
    fn parse_repo_identity_from_ssh_origin() {
        let identity = parse_repo_identity("git@github.com:typesense/typesense.git").unwrap();
        assert_eq!(identity.full(), "github.com/typesense/typesense");
    }

    #[test]
    fn exact_repo_rule_beats_wildcard() {
        let ctx = MatchContext {
            repo_full: Some("github.com/typesense/typesense".to_string()),
            repo_slug: Some("typesense/typesense".to_string()),
            paths: vec![],
        };
        let rules = vec![
            GitHubTokenRule {
                pattern: "*".to_string(),
                token_env_var: "GH_TOKEN_FULL".to_string(),
            },
            GitHubTokenRule {
                pattern: "typesense/typesense".to_string(),
                token_env_var: "GH_TOKEN_SCOPED".to_string(),
            },
        ];
        let selected = select_best_rule(&rules, &ctx).unwrap();
        assert_eq!(selected.token_env_var, "GH_TOKEN_SCOPED");
    }

    #[test]
    fn specific_wildcard_beats_generic_wildcard() {
        let ctx = MatchContext {
            repo_full: Some("github.com/typesense/server".to_string()),
            repo_slug: Some("typesense/server".to_string()),
            paths: vec![],
        };
        let rules = vec![
            GitHubTokenRule {
                pattern: "*".to_string(),
                token_env_var: "GH_TOKEN_FULL".to_string(),
            },
            GitHubTokenRule {
                pattern: "typesense/*".to_string(),
                token_env_var: "GH_TOKEN_TYPESENSE".to_string(),
            },
        ];
        let selected = select_best_rule(&rules, &ctx).unwrap();
        assert_eq!(selected.token_env_var, "GH_TOKEN_TYPESENSE");
    }

    #[test]
    fn path_rule_matches_when_repo_is_unknown() {
        let ctx = MatchContext {
            repo_full: None,
            repo_slug: None,
            paths: vec!["c:/code/typesense/repo".to_string()],
        };
        let rules = vec![GitHubTokenRule {
            pattern: "c:/code/typesense/*".to_string(),
            token_env_var: "GH_TOKEN_TYPESENSE".to_string(),
        }];
        let selected = select_best_rule(&rules, &ctx).unwrap();
        assert_eq!(selected.token_env_var, "GH_TOKEN_TYPESENSE");
    }
}
