// scripts/demo-acts.mjs
// Demo sequence definition — an ordered array of acts, each with timed steps.
//
// Step types:
//   { type: 'mcp', tool, args, delay?, storeAs? }
//   { type: 'playwright', action, ...params, delay? }
//   { type: 'pause', ms }
//   { type: 'log', message }
//
// `storeAs` saves the MCP result content into context[storeAs] for later use.
// `args` can reference context via "$varName" strings which get resolved at runtime.

const PROJECT_DIR = 'C:/Users/alanm/Documents/dev/godly-claude/godly-terminal';

export const acts = [
  // ──────────────────────────────────────────
  // Act 1: Workspace Orchestration
  // ──────────────────────────────────────────
  {
    id: 'workspace-setup',
    caption: 'Act 1: Workspace Orchestration',
    steps: [
      { type: 'log', message: 'Creating demo workspaces...' },
      {
        type: 'mcp', tool: 'create_workspace',
        args: { name: 'Backend API', folder_path: PROJECT_DIR },
        storeAs: 'backendWs', delay: 1000,
      },
      {
        type: 'mcp', tool: 'create_workspace',
        args: { name: 'Frontend App', folder_path: PROJECT_DIR },
        storeAs: 'frontendWs', delay: 800,
      },
      {
        type: 'mcp', tool: 'create_workspace',
        args: { name: 'Infrastructure', folder_path: PROJECT_DIR },
        storeAs: 'infraWs', delay: 800,
      },
      { type: 'log', message: 'Switching between workspaces...' },
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$backendWs.workspace_id' }, delay: 1200,
      },
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$frontendWs.workspace_id' }, delay: 1200,
      },
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$infraWs.workspace_id' }, delay: 1200,
      },
    ],
  },

  // ──────────────────────────────────────────
  // Act 2: Terminal Power
  // ──────────────────────────────────────────
  {
    id: 'terminal-power',
    caption: 'Act 2: Terminal Power',
    steps: [
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$backendWs.workspace_id' }, delay: 800,
      },
      { type: 'log', message: 'Spinning up terminals...' },
      {
        type: 'mcp', tool: 'create_terminal',
        args: { workspace_id: '$backendWs.workspace_id', command: "echo '>>> Backend API server starting...'" },
        storeAs: 'apiTerminal', delay: 800,
      },
      {
        type: 'mcp', tool: 'create_terminal',
        args: { workspace_id: '$backendWs.workspace_id', command: "echo '>>> Test runner ready'" },
        storeAs: 'testTerminal', delay: 800,
      },
      {
        type: 'mcp', tool: 'create_terminal',
        args: { workspace_id: '$backendWs.workspace_id', command: "echo '>>> Log watcher active'" },
        storeAs: 'logTerminal', delay: 800,
      },
      { type: 'log', message: 'Renaming terminal tabs...' },
      {
        type: 'mcp', tool: 'rename_terminal',
        args: { terminal_id: '$apiTerminal.terminal_id', name: 'API Server' }, delay: 600,
      },
      {
        type: 'mcp', tool: 'rename_terminal',
        args: { terminal_id: '$testTerminal.terminal_id', name: 'Tests' }, delay: 600,
      },
      {
        type: 'mcp', tool: 'rename_terminal',
        args: { terminal_id: '$logTerminal.terminal_id', name: 'Logs' }, delay: 600,
      },
      { type: 'log', message: 'Executing commands...' },
      {
        type: 'mcp', tool: 'execute_command',
        args: {
          terminal_id: '$apiTerminal.terminal_id',
          command: "node -e \"console.log('Server listening on port 3000')\"",
        },
        delay: 1500,
      },
      {
        type: 'mcp', tool: 'execute_command',
        args: {
          terminal_id: '$testTerminal.terminal_id',
          command: "node -e \"console.log('Running 47 tests...'); setTimeout(() => console.log('All 47 tests passed'), 500)\"",
          timeout_ms: 5000,
        },
        delay: 2000,
      },
      { type: 'log', message: 'Reading terminal output...' },
      {
        type: 'mcp', tool: 'read_terminal',
        args: { terminal_id: '$testTerminal.terminal_id', mode: 'tail', lines: 10, strip_ansi: true },
        delay: 1000,
      },
      // Note: focus_terminal calls are no longer needed here — MCP tools
      // auto-focus the terminal they act on (execute_command, write_to_terminal,
      // send_keys, create_terminal). Explicit focus_terminal is still available
      // for read-only observation or showcasing the tool itself.
    ],
  },

  // ──────────────────────────────────────────
  // Act 3: Terminal I/O & Special Keys
  // ──────────────────────────────────────────
  {
    id: 'terminal-io',
    caption: 'Act 3: Terminal I/O & Special Keys',
    steps: [
      { type: 'log', message: 'Interactive terminal control...' },
      {
        type: 'mcp', tool: 'create_terminal',
        args: {
          workspace_id: '$backendWs.workspace_id',
          command: "node -e \"setTimeout(() => console.log('BUILD COMPLETE'), 2000)\"",
        },
        storeAs: 'buildTerminal', delay: 500,
      },
      {
        type: 'mcp', tool: 'rename_terminal',
        args: { terminal_id: '$buildTerminal.terminal_id', name: 'Build' }, delay: 500,
      },
      {
        type: 'mcp', tool: 'wait_for_text',
        args: { terminal_id: '$buildTerminal.terminal_id', text: 'BUILD COMPLETE', timeout_ms: 10000 },
        delay: 1500,
      },
      { type: 'log', message: 'Sending keystrokes...' },
      {
        type: 'mcp', tool: 'write_to_terminal',
        args: { terminal_id: '$logTerminal.terminal_id', data: 'hello from Claude\n' },
        delay: 1000,
      },
      {
        type: 'mcp', tool: 'send_keys',
        args: { terminal_id: '$logTerminal.terminal_id', keys: ['ctrl+c'] },
        delay: 1000,
      },
      { type: 'log', message: 'Reading visible screen...' },
      {
        type: 'mcp', tool: 'read_grid',
        args: { terminal_id: '$logTerminal.terminal_id' },
        delay: 1500,
      },
    ],
  },

  // ──────────────────────────────────────────
  // Act 4: Quick Claude — Parallel AI Agents
  // ──────────────────────────────────────────
  {
    id: 'quick-claude',
    caption: 'Act 4: Quick Claude — Parallel AI Agents',
    steps: [
      { type: 'log', message: 'Spawning parallel Claude Code agents...' },
      {
        type: 'mcp', tool: 'quick_claude',
        args: {
          workspace_id: '$backendWs.workspace_id',
          prompt: 'Read the package.json and list all dependencies with a brief description of what each one does',
        },
        storeAs: 'qc1', delay: 1500,
      },
      {
        type: 'mcp', tool: 'quick_claude',
        args: {
          workspace_id: '$backendWs.workspace_id',
          prompt: 'Find all TODO comments in the codebase and create a prioritized list',
          branch_name: 'audit-todos',
        },
        storeAs: 'qc2', delay: 1500,
      },
      {
        type: 'mcp', tool: 'quick_claude',
        args: {
          workspace_id: '$backendWs.workspace_id',
          prompt: 'Write a comprehensive README.md for this project based on the existing code',
          branch_name: 'generate-readme',
        },
        storeAs: 'qc3', delay: 1500,
      },
      { type: 'log', message: 'Listing all terminals...' },
      {
        type: 'mcp', tool: 'list_terminals',
        args: {}, delay: 2000,
      },
    ],
  },

  // ──────────────────────────────────────────
  // Act 5: Cross-Terminal Orchestration
  // ──────────────────────────────────────────
  {
    id: 'cross-terminal',
    caption: 'Act 5: Cross-Terminal Orchestration',
    steps: [
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$frontendWs.workspace_id' }, delay: 1000,
      },
      {
        type: 'mcp', tool: 'create_terminal',
        args: { workspace_id: '$frontendWs.workspace_id', command: "echo 'Frontend dev server'" },
        storeAs: 'devServer', delay: 800,
      },
      {
        type: 'mcp', tool: 'rename_terminal',
        args: { terminal_id: '$devServer.terminal_id', name: 'Dev Server' }, delay: 600,
      },
      { type: 'log', message: 'Moving terminal to another workspace...' },
      {
        type: 'mcp', tool: 'move_terminal_to_workspace',
        args: { terminal_id: '$devServer.terminal_id', workspace_id: '$infraWs.workspace_id' },
        delay: 1200,
      },
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$infraWs.workspace_id' }, delay: 1000,
      },
      { type: 'log', message: 'Resizing terminal...' },
      {
        type: 'mcp', tool: 'resize_terminal',
        args: { terminal_id: '$devServer.terminal_id', rows: 50, cols: 120 },
        delay: 1000,
      },
      {
        type: 'mcp', tool: 'notify',
        args: { message: 'Demo checkpoint: All systems operational' },
        delay: 1500,
      },
    ],
  },

  // ──────────────────────────────────────────
  // Act 6: Git Worktree Integration
  // ──────────────────────────────────────────
  {
    id: 'git-worktrees',
    caption: 'Act 6: Git Worktree Integration',
    steps: [
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$backendWs.workspace_id' }, delay: 800,
      },
      { type: 'log', message: 'Creating terminal with git worktree...' },
      {
        type: 'mcp', tool: 'create_terminal',
        args: {
          workspace_id: '$backendWs.workspace_id',
          worktree: true,
          command: 'git log --oneline -5',
        },
        storeAs: 'wtTerminal', delay: 1500,
      },
      {
        type: 'mcp', tool: 'rename_terminal',
        args: { terminal_id: '$wtTerminal.terminal_id', name: 'Feature Branch' }, delay: 600,
      },
      {
        type: 'mcp', tool: 'create_terminal',
        args: {
          workspace_id: '$backendWs.workspace_id',
          worktree_name: 'demo-feature-branch',
          command: 'git branch --show-current && echo --- && ls',
        },
        storeAs: 'namedWtTerminal', delay: 1500,
      },
      {
        type: 'mcp', tool: 'rename_terminal',
        args: { terminal_id: '$namedWtTerminal.terminal_id', name: 'demo-feature' }, delay: 600,
      },
      { type: 'pause', ms: 2000 },
    ],
  },

  // ──────────────────────────────────────────
  // Act 7: Session Persistence (narrative only)
  // ──────────────────────────────────────────
  {
    id: 'session-persistence',
    caption: 'Act 7: Session Persistence',
    steps: [
      {
        type: 'mcp', tool: 'execute_command',
        args: {
          terminal_id: '$apiTerminal.terminal_id',
          command: "node -e \"setInterval(() => console.log('heartbeat ' + Date.now()), 2000)\"",
        },
        delay: 3000,
      },
      { type: 'log', message: 'Persistence demo — terminals survive app restarts' },
      { type: 'pause', ms: 3000 },
      {
        type: 'mcp', tool: 'send_keys',
        args: { terminal_id: '$apiTerminal.terminal_id', keys: ['ctrl+c'] },
        delay: 500,
      },
    ],
  },

  // ──────────────────────────────────────────
  // Act 8: Phone Remote Control
  // ──────────────────────────────────────────
  {
    id: 'phone-remote',
    caption: 'Act 8: Phone Remote Control',
    steps: [
      { type: 'log', message: 'Opening phone remote UI...' },
      { type: 'playwright', action: 'goto', url: 'http://localhost:3377/phone', delay: 3000 },
      { type: 'playwright', action: 'screenshot', name: '01-phone-login', delay: 1500 },
      // Try login if password field visible
      {
        type: 'playwright', action: 'fill-if-visible',
        selector: 'input[type=password]', value: '', delay: 500,
      },
      { type: 'playwright', action: 'screenshot', name: '02-phone-dashboard', delay: 2000 },
      // Scroll to show workspaces
      { type: 'playwright', action: 'scroll', y: 200, delay: 1000 },
      { type: 'playwright', action: 'screenshot', name: '03-phone-scrolled', delay: 1000 },
      // Tap first session
      {
        type: 'playwright', action: 'tap-if-visible',
        selector: '.session-row:first-child, [class*=session]:first-child, [class*=terminal-row]:first-child',
        delay: 2000,
      },
      { type: 'playwright', action: 'screenshot', name: '04-phone-session', delay: 1500 },
      // Type a command
      {
        type: 'playwright', action: 'fill-if-visible',
        selector: 'input[type=text], input[placeholder*="command"]',
        value: 'echo "Hello from phone!"', delay: 1000,
      },
      { type: 'playwright', action: 'screenshot', name: '05-phone-typing', delay: 1000 },
      // Tap send
      {
        type: 'playwright', action: 'tap-if-visible',
        selector: 'button:has-text("Send"), button:has-text("Enter"), .send-btn',
        delay: 2000,
      },
      // Quick action buttons
      {
        type: 'playwright', action: 'tap-if-visible',
        selector: 'button:has-text("y")', delay: 500,
      },
      {
        type: 'playwright', action: 'tap-if-visible',
        selector: 'button:has-text("Ctrl+C")', delay: 500,
      },
      { type: 'playwright', action: 'screenshot', name: '06-phone-actions', delay: 1000 },
      // Go back
      {
        type: 'playwright', action: 'tap-if-visible',
        selector: 'button:has-text("Back"), button:has-text("<<"), .back-btn',
        delay: 1500,
      },
      // Settings
      {
        type: 'playwright', action: 'tap-if-visible',
        selector: 'button:has-text("Settings"), button:has-text("gear"), .settings-btn',
        delay: 1500,
      },
      { type: 'playwright', action: 'screenshot', name: '07-phone-settings', delay: 1500 },
    ],
  },

  // ──────────────────────────────────────────
  // Act 9: Grand Orchestra
  // ──────────────────────────────────────────
  {
    id: 'grand-orchestra',
    caption: 'Act 9: Full Orchestra',
    steps: [
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$backendWs.workspace_id' }, delay: 1000,
      },
      { type: 'log', message: 'Deploying parallel agents across workspaces...' },
      {
        type: 'mcp', tool: 'quick_claude',
        args: {
          workspace_id: '$backendWs.workspace_id',
          prompt: 'Analyze the Cargo.toml dependency tree and suggest optimizations',
        },
        delay: 1000,
      },
      {
        type: 'mcp', tool: 'quick_claude',
        args: {
          workspace_id: '$backendWs.workspace_id',
          prompt: 'Find all unwrap() calls and assess panic risk',
          branch_name: 'audit-unwraps',
        },
        delay: 1000,
      },
      {
        type: 'mcp', tool: 'switch_workspace',
        args: { workspace_id: '$frontendWs.workspace_id' }, delay: 1000,
      },
      {
        type: 'mcp', tool: 'quick_claude',
        args: {
          workspace_id: '$frontendWs.workspace_id',
          prompt: 'Audit all event listeners for memory leaks',
        },
        delay: 1000,
      },
      {
        type: 'mcp', tool: 'quick_claude',
        args: {
          workspace_id: '$frontendWs.workspace_id',
          prompt: 'Generate TypeScript type definitions from the IPC commands',
        },
        delay: 1500,
      },
      {
        type: 'mcp', tool: 'list_terminals',
        args: {}, delay: 2000,
      },
      {
        type: 'mcp', tool: 'notify',
        args: { message: '5 parallel Claude agents deployed across 2 workspaces' },
        delay: 2000,
      },
    ],
  },

  // ──────────────────────────────────────────
  // Act 10: Cleanup
  // ──────────────────────────────────────────
  {
    id: 'cleanup',
    caption: 'Act 10: Cleanup',
    steps: [
      { type: 'log', message: 'Cleaning up demo resources...' },
      // Close demo terminals (list first, then close visible ones)
      { type: 'mcp', tool: 'list_terminals', args: {}, storeAs: 'allTerminals', delay: 500 },
      // We'll close terminals tracked in context during the cleanup executor
      { type: 'cleanup-terminals', delay: 500 },
      // Delete demo workspaces
      {
        type: 'mcp', tool: 'delete_workspace',
        args: { workspace_id: '$frontendWs.workspace_id' }, delay: 800,
      },
      {
        type: 'mcp', tool: 'delete_workspace',
        args: { workspace_id: '$infraWs.workspace_id' }, delay: 800,
      },
      { type: 'log', message: 'Demo complete!' },
      { type: 'pause', ms: 2000 },
    ],
  },
];
