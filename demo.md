# Godly Terminal - MVP Demo Script

> A step-by-step script for Claude Code to execute via MCP, showcasing every major feature of Godly Terminal. Designed for a live demo or screen recording.

## Prerequisites

- Godly Terminal running (`pnpm tauri dev` or production build)
- Claude Code connected with `godly-terminal` MCP server
- Node.js installed (for Playwright mobile demo)
- `pnpm exec playwright install chromium` (one-time)

---

## Act 1: Workspace Orchestration

**Goal**: Show that Godly Terminal is built for multi-project, multi-agent workflows.

### 1.1 — Create demo workspaces

```
Use MCP: create_workspace
  name: "Backend API"
  folder_path: "C:/Users/alanm/Documents/dev/godly-claude/godly-terminal"

Use MCP: create_workspace
  name: "Frontend App"
  folder_path: "C:/Users/alanm/Documents/dev/godly-claude/godly-terminal"

Use MCP: create_workspace
  name: "Infrastructure"
  folder_path: "C:/Users/alanm/Documents/dev/godly-claude/godly-terminal"
```

### 1.2 — Show workspace switching

```
Use MCP: list_workspaces
  → Note the IDs

Use MCP: switch_workspace → "Backend API"
  (pause 1s for audience to see the switch)

Use MCP: switch_workspace → "Frontend App"
  (pause 1s)

Use MCP: switch_workspace → "Infrastructure"
```

**Talking point**: "Each workspace is an isolated project context. Switch between them instantly — no tab clutter."

---

## Act 2: Terminal Power

**Goal**: Show terminal creation, naming, execution, and multi-terminal management.

### 2.1 — Spin up terminals in the "Backend API" workspace

```
Use MCP: switch_workspace → "Backend API"

Use MCP: create_terminal
  workspace_id: <backend-api-id>
  command: "echo '🚀 Backend API server starting...'"

Use MCP: create_terminal
  workspace_id: <backend-api-id>
  command: "echo '🧪 Test runner ready'"

Use MCP: create_terminal
  workspace_id: <backend-api-id>
  command: "echo '📊 Log watcher active'"
```

### 2.2 — Rename tabs for clarity

```
Use MCP: rename_terminal → terminal 1 → "API Server"
Use MCP: rename_terminal → terminal 2 → "Tests"
Use MCP: rename_terminal → terminal 3 → "Logs"
```

### 2.3 — Execute commands and read output

```
Use MCP: execute_command (terminal "API Server")
  command: "node -e \"console.log('Server listening on port 3000')\""

Use MCP: execute_command (terminal "Tests")
  command: "node -e \"console.log('Running 47 tests...'); setTimeout(() => console.log('✓ All 47 tests passed'), 500)\""
  timeout_ms: 5000

Use MCP: execute_command (terminal "Logs")
  command: "node -e \"setInterval(() => console.log(new Date().toISOString() + ' [INFO] Request processed'), 1000)\""
```

### 2.4 — Read terminal output

```
Use MCP: read_terminal (terminal "Tests")
  mode: "tail"
  lines: 10
  strip_ansi: true
```

### 2.5 — Focus switching

```
Use MCP: focus_terminal → "API Server"
  (pause 1s)
Use MCP: focus_terminal → "Tests"
  (pause 1s)
Use MCP: focus_terminal → "Logs"
```

**Talking point**: "Claude Code can create terminals, name them, run commands, and read their output — all programmatically. This is how AI agents orchestrate real development workflows."

---

## Act 3: Terminal I/O & Special Keys

**Goal**: Demonstrate interactive terminal control.

### 3.1 — Interactive command with wait_for_text

```
Use MCP: create_terminal
  workspace_id: <backend-api-id>
  command: "node -e \"setTimeout(() => console.log('BUILD COMPLETE'), 2000)\""

Use MCP: rename_terminal → "Build"

Use MCP: wait_for_text
  terminal_id: <build-terminal>
  text: "BUILD COMPLETE"
  timeout_ms: 10000
```

### 3.2 — Send special keys

```
Use MCP: execute_command (terminal "Logs")
  command: "node -e \"process.stdout.write('Type something: '); process.stdin.resume()\""

Use MCP: write_to_terminal (terminal "Logs")
  data: "hello from Claude"

Use MCP: send_keys (terminal "Logs")
  keys: ["enter"]

(pause 1s)

Use MCP: send_keys (terminal "Logs")
  keys: ["ctrl+c"]
```

### 3.3 — Read the visible screen

```
Use MCP: read_grid (terminal "Logs")
  → Shows exactly what the user sees, with cursor position
```

**Talking point**: "Full interactive control — send keystrokes, wait for specific output, read the exact screen state. Claude Code has the same terminal access as a human developer."

---

## Act 4: Quick Claude — Parallel AI Agents

**Goal**: Show the killer feature — spawning multiple Claude Code instances instantly.

### 4.1 — Fire off parallel agents

```
Use MCP: spawn_claude_session
  workspace_id: <backend-api-id>
  prompt: "Read the package.json and list all dependencies with a brief description of what each one does"

Use MCP: spawn_claude_session
  workspace_id: <backend-api-id>
  prompt: "Find all TODO comments in the codebase and create a prioritized list"
  branch_name: "audit-todos"

Use MCP: spawn_claude_session
  workspace_id: <backend-api-id>
  prompt: "Write a comprehensive README.md for this project based on the existing code"
  branch_name: "generate-readme"
```

### 4.2 — Show all terminals

```
Use MCP: list_terminals
  → Shows all terminals including the spawn_claude_session ones with their worktree branches
```

**Talking point**: "One command spawns a new Claude Code session in an isolated git worktree. Fire 3 in parallel — each works on its own branch, its own copy of the repo, zero conflicts. This is how you turn a 3-hour task into a 20-minute one."

---

## Act 5: Cross-Terminal Orchestration

**Goal**: Show moving terminals between workspaces and managing the full fleet.

### 5.1 — Move a terminal to another workspace

```
Use MCP: switch_workspace → "Frontend App"

Use MCP: create_terminal
  workspace_id: <frontend-id>
  command: "echo 'Frontend dev server'"

Use MCP: rename_terminal → "Dev Server"

Use MCP: move_terminal_to_workspace
  terminal_id: <dev-server-id>
  workspace_id: <infrastructure-id>

Use MCP: switch_workspace → "Infrastructure"
  → Dev Server terminal is now here
```

### 5.2 — Resize a terminal

```
Use MCP: resize_terminal
  terminal_id: <any-terminal>
  rows: 50
  cols: 120
```

### 5.3 — Notifications

```
Use MCP: notify
  message: "Demo checkpoint: All systems operational"
```

**Talking point**: "Move terminals between workspaces, resize them programmatically, get notifications when tasks complete. Everything is orchestratable."

---

## Act 6: Git Worktree Integration

**Goal**: Show native worktree support for parallel development.

### 6.1 — Create terminal with worktree

```
Use MCP: create_terminal
  workspace_id: <backend-api-id>
  worktree: true
  command: "git log --oneline -5"

Use MCP: rename_terminal → "Feature Branch"
```

### 6.2 — Create terminal with named worktree

```
Use MCP: create_terminal
  workspace_id: <backend-api-id>
  worktree_name: "demo-feature-branch"
  command: "git branch --show-current && echo '---' && ls"

Use MCP: rename_terminal → "demo-feature"
```

### 6.3 — Clean up worktree

```
Use MCP: remove_worktree
  worktree_path: <worktree-path-from-create>
```

**Talking point**: "First-class git worktree support. Every parallel Claude Code instance gets its own worktree automatically — isolated branches, no stashing, no conflicts."

---

## Act 7: Session Persistence

**Goal**: Show that terminals survive app restarts.

### 7.1 — Create a long-running process

```
Use MCP: execute_command (terminal "API Server")
  command: "node -e \"setInterval(() => console.log('heartbeat ' + Date.now()), 2000)\""
```

### 7.2 — Narrate the persistence model

**Talking point**: "Close Godly Terminal. The daemon keeps every session alive in the background. Reopen the app — all terminals reconnect instantly. Your AI agents never lose their sessions, even across restarts. This is tmux-level persistence, built into a native app."

---

## Act 8: The Phone — Mobile Remote Control

**Goal**: Show the phone remote for approving Claude Code prompts from your pocket. This is the "wow" moment.

### 8.1 — Start the phone remote server

```
Use MCP: execute_command (in any terminal)
  command: "pwsh scripts/setup-phone.ps1"
  timeout_ms: 30000

→ Wait for QR code to appear
→ Note the ngrok URL (e.g., https://xxxx.ngrok-free.app)
```

### 8.2 — Show the phone UI with Playwright (mobile viewport)

Create and run this Playwright script to simulate the phone experience on screen:

```
Use MCP: execute_command (in a new terminal)
  command: |
    node -e "
    const { chromium } = require('playwright');
    (async () => {
      const browser = await chromium.launch({ headless: false });
      const context = await browser.newContext({
        viewport: { width: 390, height: 844 },
        deviceScaleFactor: 3,
        isMobile: true,
        hasTouch: true,
        userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1'
      });
      const page = await context.newPage();

      // Go to the phone UI (local — skip ngrok for demo)
      await page.goto('http://localhost:3377/phone');
      console.log('PHONE_UI_LOADED');

      // Keep browser open for demo
      await new Promise(() => {});
    })();
    "
  timeout_ms: 120000
```

Wait for `PHONE_UI_LOADED`, then proceed with the interactive demo.

### 8.3 — Playwright: Navigate the phone UI

Run a second script that interacts with the phone UI while it's open:

```
Use MCP: execute_command (in another terminal)
  command: |
    node -e "
    const { chromium } = require('playwright');
    (async () => {
      // Connect to the existing browser
      const browser = await chromium.launch({ headless: false });
      const context = await browser.newContext({
        viewport: { width: 390, height: 844 },
        deviceScaleFactor: 3,
        isMobile: true,
        hasTouch: true,
        userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X)'
      });
      const page = await context.newPage();
      await page.goto('http://localhost:3377/phone');

      // --- LOGIN ---
      console.log('STEP: Login screen');
      await page.screenshot({ path: 'demo-screenshots/01-phone-login.png' });
      await page.waitForTimeout(2000);

      // Enter password (the one from setup-phone.ps1 output)
      const passwordInput = page.locator('input[type=password]');
      if (await passwordInput.isVisible()) {
        await passwordInput.fill('demo-password-here');
        await passwordInput.press('Enter');
        await page.waitForTimeout(1000);
      }

      // --- DASHBOARD ---
      console.log('STEP: Dashboard view');
      await page.screenshot({ path: 'demo-screenshots/02-phone-dashboard.png' });
      await page.waitForTimeout(3000);

      // Scroll through workspaces
      await page.evaluate(() => window.scrollBy(0, 300));
      await page.waitForTimeout(1000);
      await page.screenshot({ path: 'demo-screenshots/03-phone-workspaces.png' });

      // --- TAP A TERMINAL ---
      console.log('STEP: Opening a terminal session');
      const terminalRow = page.locator('.session-row').first();
      if (await terminalRow.isVisible()) {
        await terminalRow.tap();
        await page.waitForTimeout(2000);
        await page.screenshot({ path: 'demo-screenshots/04-phone-session.png' });
      }

      // --- SHOW QUICK INPUT BUTTONS ---
      console.log('STEP: Quick input buttons');
      await page.screenshot({ path: 'demo-screenshots/05-phone-input-bar.png' });

      // Tap 'y' quick button
      const yButton = page.locator('button:has-text(\"y\")').first();
      if (await yButton.isVisible()) {
        await yButton.tap();
        await page.waitForTimeout(500);
      }

      // Tap Ctrl+C
      const ctrlCButton = page.locator('button:has-text(\"Ctrl+C\")').first();
      if (await ctrlCButton.isVisible()) {
        await ctrlCButton.tap();
        await page.waitForTimeout(500);
      }

      // --- TYPE A COMMAND ---
      console.log('STEP: Typing a command');
      const inputField = page.locator('input[type=text]').first();
      if (await inputField.isVisible()) {
        await inputField.tap();
        await inputField.fill('echo hello from phone');
        await page.screenshot({ path: 'demo-screenshots/06-phone-typing.png' });
        await page.waitForTimeout(1000);

        // Tap send
        const sendButton = page.locator('button:has-text(\"↵\")').first();
        if (await sendButton.isVisible()) {
          await sendButton.tap();
          await page.waitForTimeout(2000);
        }
      }

      // --- GO BACK TO DASHBOARD ---
      console.log('STEP: Back to dashboard');
      const backButton = page.locator('button:has-text(\"←\")').first();
      if (await backButton.isVisible()) {
        await backButton.tap();
        await page.waitForTimeout(1000);
      }
      await page.screenshot({ path: 'demo-screenshots/07-phone-dashboard-final.png' });

      // --- SETTINGS ---
      console.log('STEP: Settings view');
      const settingsButton = page.locator('button:has-text(\"⚙\")').first();
      if (await settingsButton.isVisible()) {
        await settingsButton.tap();
        await page.waitForTimeout(1000);
        await page.screenshot({ path: 'demo-screenshots/08-phone-settings.png' });
      }

      console.log('DEMO_COMPLETE');
      await page.waitForTimeout(5000);
      await browser.close();
    })();
    "
  timeout_ms: 120000
```

### 8.4 — Live phone approval flow

While Playwright shows the phone UI, trigger a prompt in a terminal:

```
Use MCP: execute_command (terminal "API Server")
  command: "read -p 'Do you want to proceed? [Y/n] ' answer && echo \"You chose: $answer\""

→ The phone UI should show a prompt alert card with Approve/Deny buttons
→ Use Playwright to tap "Approve" or demonstrate tapping on the physical phone
```

### 8.5 — Phone + Claude Code approval flow

The real killer demo — Claude Code asks for permission, you approve from your phone:

```
Use MCP: spawn_claude_session
  workspace_id: <backend-api-id>
  prompt: "Create a new file called demo-test.txt with the contents 'Hello from the demo'"

→ Claude Code will ask for file write permission
→ Phone dashboard shows the prompt alert in real-time (SSE)
→ Tap Approve on phone
→ Claude Code continues and creates the file
```

**Talking point**: "You're at lunch. Claude Code is working on your project. Your phone buzzes — 'Claude wants to write a file.' One tap: approved. No laptop needed. This is the future of AI-assisted development."

---

## Act 9: Full Orchestra — Putting It All Together

**Goal**: Show the complete workflow — multiple workspaces, parallel agents, phone control.

### 9.1 — The grand orchestration

```
# Switch to Backend API workspace
Use MCP: switch_workspace → "Backend API"

# Fire 3 parallel Claude agents
Use MCP: spawn_claude_session → "Analyze the Cargo.toml dependency tree and suggest optimizations"
Use MCP: spawn_claude_session → "Find all unwrap() calls and assess panic risk" branch: "audit-unwraps"
Use MCP: spawn_claude_session → "Generate integration test stubs for untested modules" branch: "gen-tests"

# Switch to Frontend workspace and fire 2 more
Use MCP: switch_workspace → "Frontend App"
Use MCP: spawn_claude_session → "Audit all event listeners for memory leaks"
Use MCP: spawn_claude_session → "Generate TypeScript type definitions from the IPC commands"

# List everything running
Use MCP: list_terminals
  → Shows 10+ terminals across 3 workspaces, each doing real work

# Send notification
Use MCP: notify
  message: "5 parallel Claude agents deployed across 2 workspaces"
```

**Talking point**: "5 Claude Code instances, 3 workspaces, each on isolated git branches. All spawned in under 10 seconds. Monitor and approve from your phone. This isn't a terminal — it's a command center for AI-powered development."

---

## Act 10: Cleanup

```
# Close demo terminals (keep it tidy)
Use MCP: list_terminals → get all IDs

# Close each one
Use MCP: close_terminal → <each-demo-terminal>

# Delete demo workspaces
Use MCP: delete_workspace → "Frontend App"
Use MCP: delete_workspace → "Infrastructure"

# Keep Backend API as the main workspace
```

---

## Appendix A: Mobile Demo — Standalone Playwright Script

Save this as `scripts/demo-phone.mjs` for a self-contained phone UI demo:

```javascript
// scripts/demo-phone.mjs
// Run: node scripts/demo-phone.mjs [phone-url] [password]
//
// If no URL provided, defaults to http://localhost:3377/phone
// Requires: npx playwright install chromium

import { chromium } from 'playwright';
import { mkdirSync } from 'fs';

const PHONE_URL = process.argv[2] || 'http://localhost:3377/phone';
const PASSWORD = process.argv[3] || '';

mkdirSync('demo-screenshots', { recursive: true });

const iPhone14Pro = {
  viewport: { width: 393, height: 852 },
  deviceScaleFactor: 3,
  isMobile: true,
  hasTouch: true,
  userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
};

async function sleep(ms) {
  return new Promise(r => setTimeout(r, ms));
}

async function screenshot(page, name) {
  const path = `demo-screenshots/${name}.png`;
  await page.screenshot({ path, fullPage: false });
  console.log(`  📸 ${path}`);
}

(async () => {
  console.log('🚀 Starting Godly Terminal Phone Demo');
  console.log(`   URL: ${PHONE_URL}`);
  console.log('');

  const browser = await chromium.launch({
    headless: false,
    args: ['--window-size=430,932'],
  });
  const context = await browser.newContext(iPhone14Pro);
  const page = await context.newPage();

  // ─── Login ───
  console.log('📱 Step 1: Opening phone UI...');
  await page.goto(PHONE_URL);
  await sleep(2000);
  await screenshot(page, '01-login');

  if (PASSWORD) {
    console.log('🔑 Step 2: Entering password...');
    const pwInput = page.locator('input[type=password]');
    if (await pwInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await pwInput.tap();
      await sleep(300);
      await pwInput.fill(PASSWORD);
      await sleep(500);
      await screenshot(page, '02-password-entered');
      await pwInput.press('Enter');
      await sleep(2000);
    }
  }

  // ─── Dashboard ───
  console.log('📊 Step 3: Dashboard view...');
  await screenshot(page, '03-dashboard');
  await sleep(1500);

  // Scroll to show workspaces
  await page.evaluate(() => window.scrollBy(0, 200));
  await sleep(800);
  await screenshot(page, '04-dashboard-scrolled');

  // Check for prompt alerts
  const promptCards = page.locator('.prompt-card, .alert-card, [class*=prompt]');
  const promptCount = await promptCards.count();
  if (promptCount > 0) {
    console.log(`🔔 Found ${promptCount} prompt alert(s)!`);
    await screenshot(page, '05-prompt-alerts');

    // Tap approve on the first prompt
    const approveBtn = page.locator('button:has-text("Approve"), button:has-text("Yes"), .approve-btn').first();
    if (await approveBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
      console.log('✅ Tapping Approve...');
      await approveBtn.tap();
      await sleep(1500);
      await screenshot(page, '06-prompt-approved');
    }
  }

  // ─── Open a terminal session ───
  console.log('💻 Step 4: Opening a terminal session...');
  const sessions = page.locator('[class*=session], [class*=terminal-row], .session-row');
  const sessionCount = await sessions.count();
  if (sessionCount > 0) {
    await sessions.first().tap();
    await sleep(2000);
    await screenshot(page, '07-session-view');

    // Show the input bar and quick buttons
    console.log('⌨️  Step 5: Interacting with terminal...');

    // Type a command
    const textInput = page.locator('input[type=text], input[placeholder*="command"], input[placeholder*="input"]').first();
    if (await textInput.isVisible({ timeout: 2000 }).catch(() => false)) {
      await textInput.tap();
      await sleep(300);
      await textInput.fill('echo "Hello from phone! 📱"');
      await screenshot(page, '08-typing-command');
      await sleep(800);

      // Tap send
      const sendBtn = page.locator('button:has-text("↵"), button:has-text("Send"), .send-btn').first();
      if (await sendBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
        await sendBtn.tap();
        await sleep(2000);
        await screenshot(page, '09-command-sent');
      }
    }

    // Tap quick action buttons
    for (const key of ['y', 'Enter', 'Ctrl+C']) {
      const btn = page.locator(`button:has-text("${key}")`).first();
      if (await btn.isVisible({ timeout: 500 }).catch(() => false)) {
        console.log(`   Tapping quick button: ${key}`);
        await btn.tap();
        await sleep(500);
      }
    }
    await screenshot(page, '10-quick-buttons');

    // Scroll terminal output
    const outputArea = page.locator('[class*=output], [class*=terminal-text], pre, code').first();
    if (await outputArea.isVisible({ timeout: 1000 }).catch(() => false)) {
      await outputArea.evaluate(el => el.scrollTop = el.scrollHeight);
      await sleep(500);
      await screenshot(page, '11-scrolled-output');
    }

    // Go back to dashboard
    const backBtn = page.locator('button:has-text("←"), button:has-text("Back"), .back-btn').first();
    if (await backBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
      await backBtn.tap();
      await sleep(1000);
    }
  }

  // ─── Settings ───
  console.log('⚙️  Step 6: Settings view...');
  const settingsBtn = page.locator('button:has-text("⚙"), button:has-text("Settings"), .settings-btn').first();
  if (await settingsBtn.isVisible({ timeout: 2000 }).catch(() => false)) {
    await settingsBtn.tap();
    await sleep(1500);
    await screenshot(page, '12-settings');

    // Go back
    const backBtn2 = page.locator('button:has-text("←"), button:has-text("Back"), .back-btn').first();
    if (await backBtn2.isVisible({ timeout: 1000 }).catch(() => false)) {
      await backBtn2.tap();
      await sleep(1000);
    }
  }

  // ─── Final dashboard state ───
  console.log('📊 Step 7: Final dashboard state...');
  await screenshot(page, '13-final-dashboard');

  console.log('');
  console.log('✅ Demo complete! Screenshots saved to demo-screenshots/');
  console.log('   Press Ctrl+C to close the browser.');
  console.log('');

  // Keep browser open for live demo
  await new Promise(() => {});
})();
```

## Appendix B: Phone Demo — Manual QR Flow (for live audience)

If doing a live demo with a real phone:

1. Run `pwsh scripts/setup-phone.ps1` in a Godly Terminal tab
2. QR code appears — point camera at it
3. Phone opens the dashboard in Safari/Chrome
4. Enter the password shown in the terminal
5. Fire a `spawn_claude_session` agent that will ask for permissions
6. Show the phone receiving the prompt notification in real-time
7. Tap "Approve" on the phone
8. Show the terminal continuing on the desktop

## Appendix C: Demo Timing Guide

| Act | Duration | Highlight |
|-----|----------|-----------|
| 1. Workspaces | 30s | Instant switching |
| 2. Terminals | 45s | Create, name, execute, read |
| 3. Terminal I/O | 30s | Special keys, wait_for_text, read_grid |
| 4. Quick Claude | 45s | Parallel agent spawning |
| 5. Cross-Terminal | 30s | Move terminals, resize, notify |
| 6. Git Worktrees | 30s | Isolated branches per agent |
| 7. Persistence | 15s | Narrative (survive restarts) |
| 8. Phone Remote | 90s | The showstopper |
| 9. Grand Orchestra | 45s | Everything together |
| 10. Cleanup | 15s | Tidy exit |
| **Total** | **~6 min** | |

## Appendix D: Key Stats to Mention

- **27 MCP tools** — full terminal control from any AI agent
- **20+ concurrent sessions** — no degradation
- **Session persistence** — daemon survives app restarts (tmux-level)
- **Git worktrees** — one-command isolated branches for parallel work
- **Phone remote** — approve AI actions from your pocket via QR code
- **Real-time SSE** — instant prompt notifications on phone
- **5 shell types** — PowerShell, Pwsh, Cmd, WSL, Custom
- **Canvas2D renderer** — efficient, hardware-accelerated terminal display
- **SIMD VT parser** — godly-vt with image protocol support (Kitty/iTerm2/Sixel)
