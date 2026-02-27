# Claude Code Skills & Hooks

All custom skills and automation configured for this project.

## Project Skills

These are defined in `.claude/settings.json` and scoped to godly-terminal.

| Skill | Usage | Purpose |
|-------|-------|---------|
| `/build` | `/build [dev\|prod\|preview]` | Build and run Godly Terminal in development or production mode |
| `/component` | `/component <Name> [desc]` | Generate a new TypeScript UI component following the project's vanilla DOM + Canvas2D pattern |
| `/feature` | `/feature <name> [desc]` | Implement a new feature end-to-end: Rust backend command + TypeScript frontend service + state + UI |
| `/tauri-command` | `/tauri-command <name> [desc]` | Scaffold the full Tauri IPC chain: Rust handler + register in lib.rs + TypeScript service wrapper |
| `/tidy-up-docs` | `/tidy-up-docs` | Audit `docs/`, classify status (resolved/active/reference), build index, distill lessons into MEMORY.md, archive stale docs |
| `/manual-testing` | `/manual-testing <feature>` | QA a feature via MCP tools: design test matrix, execute tests, take screenshots, analyze UX, file a GitHub issue with all findings |
| `/fix-integration` | `/fix-integration [filter]` | Run integration tests in a loop, diagnose and fix real failures until green (max 5 iterations). Never masks failures. |

**Unregistered** (in `skills/` but not in `settings.json`):

| Skill | Purpose |
|-------|---------|
| `/figma-design` | Create/modify Figma designs via Playwright + Plugin API (bypasses MCP call limits) |
| `/bump-version` | Bump the project version and create a git tag |

## User-Scoped Skills (Plugins)

These are installed globally via `~/.claude/settings.json` and available in all projects.

| Skill | Source | Purpose |
|-------|--------|---------|
| `/commit` | commit-commands plugin | Create a git commit with conventional format |
| `/commit-push-pr` | commit-commands plugin | Commit, push, and open a PR in one command |
| `/clean_gone` | commit-commands plugin | Clean up local branches whose remote was deleted (`[gone]`), including worktrees |
| `/revise-claude-md` | claude-md-management plugin | Update CLAUDE.md with learnings from the current session |
| `/claude-md-improver` | claude-md-management plugin | Audit and improve CLAUDE.md files — scan, evaluate quality, make targeted updates |
| `/frontend-design` | frontend-design plugin | Create production-grade frontend UI with high design quality |
| `/skill-creator` | skill-creator plugin | Create new skills, improve existing ones, run evals and benchmarks |
| `/learn` | (built-in) | Analyze the conversation for mistakes or lessons learned, save to memory |
| `/test-hygiene` | (built-in) | Analyze a test suite for quality, validity, output noise, and test smells |
| `/support-ticket` | (built-in) | Fact-check a support ticket question against the codebase |
| `/create-reproducible-issue` | (built-in) | Write a test suite that reproduces a bug from a pasted report |
| `/performance-analysis` | (built-in) | Analyze a Typesense collection schema for performance issues |
| `/keybindings-help` | (built-in) | Customize keyboard shortcuts and chord bindings |
| `/release` | (built-in) | Merge open PRs, update changelog, bump version |
| `/bump-version` | (built-in) | Bump version and create a git tag |

## Hooks

Configured in `.claude/settings.json` (project) and `~/.claude/settings.json` (global).

### Project Hooks

| Event | Script | Behavior |
|-------|--------|----------|
| `Stop` | `scripts/post-implementation-reminder.sh` | After each Claude response, checks for uncommitted code changes (`.rs`, `.ts`, `.js`, `.css`, `.html`). If found, suggests relevant skills: testing, manual-testing, commit, learn. Silent when only docs/config changed. |

### Global Hooks

| Event | Command | Behavior |
|-------|---------|----------|
| `Stop` | `godly-notify.exe "Claude finished"` | Plays a sound notification via Godly Terminal when Claude finishes responding |

## Custom Agents

Defined in `.claude/agents/` for use with the Task tool (not user-invocable).

| Agent | Purpose |
|-------|---------|
| `build-validator` | Run verification suite after code changes |
| `daemon-specialist` | Daemon, PTY sessions, named pipe IPC, ring buffers |
| `frontend-specialist` | Canvas2D rendering, vanilla DOM components, observable store |
| `issue-tracker` | GitHub Issue lifecycle management |
| `mcp-specialist` | godly-mcp changes, MCP pipe server, testing |
| `orchestrator` | Coordinate parallel Claude Code instances via godly-mcp |
| `perf-investigator` | Performance investigation, benchmarking, profiling |
