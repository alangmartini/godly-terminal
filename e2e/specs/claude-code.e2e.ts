import {
  waitForAppReady,
  waitForTerminalPane,
  getElementCount,
  elementExists,
  createNewTerminalTab,
} from '../helpers/app';
import { waitForTerminalText } from '../helpers/terminal-reader';

/**
 * Invoke a Tauri command from the browser context.
 */
async function invokeCommand<T>(cmd: string, args: Record<string, unknown> = {}): Promise<T> {
  const result = await browser.executeAsync(
    (command: string, commandArgs: Record<string, unknown>, done: (r: any) => void) => {
      const invoke = (window as any).__TAURI__?.core?.invoke;
      if (!invoke) {
        done({ error: '__TAURI__ not available' });
        return;
      }
      invoke(command, commandArgs)
        .then((r: any) => done({ ok: r }))
        .catch((e: any) => done({ error: String(e) }));
    },
    cmd,
    args
  );
  if (result && typeof result === 'object' && 'error' in result) {
    throw new Error(`invoke ${cmd} failed: ${result.error}`);
  }
  return (result as any).ok;
}

/**
 * Toggle Claude Code mode for a workspace via IPC and update the frontend store.
 */
async function toggleClaudeCodeMode(workspaceId: string, enabled: boolean): Promise<void> {
  await invokeCommand('toggle_claude_code_mode', {
    workspaceId,
    enabled,
  });
  await browser.execute(
    (wsId: string, mode: boolean) => {
      const appStore = (window as any).__store;
      if (appStore) {
        appStore.updateWorkspace(wsId, { claudeCodeMode: mode });
      }
    },
    workspaceId,
    enabled
  );
  await browser.pause(500);
}

/**
 * Get the active workspace ID from the store.
 */
async function getActiveWorkspaceId(): Promise<string | null> {
  return browser.execute(() => {
    const appStore = (window as any).__store;
    return appStore ? appStore.getState().activeWorkspaceId : null;
  });
}

/**
 * Get the claudeCodeMode flag for a workspace from the store.
 */
async function getClaudeCodeMode(workspaceId: string): Promise<boolean> {
  return browser.execute((wsId: string) => {
    const appStore = (window as any).__store;
    if (!appStore) return false;
    const ws = appStore.getState().workspaces.find((w: any) => w.id === wsId);
    return ws ? ws.claudeCodeMode : false;
  }, workspaceId);
}

describe('Claude Code Mode', () => {
  let workspaceId: string;

  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();

    workspaceId = (await getActiveWorkspaceId())!;
    expect(workspaceId).toBeTruthy();
  });

  after(async () => {
    // Ensure CC mode is off after tests
    await toggleClaudeCodeMode(workspaceId, false);
  });

  it('should toggle Claude Code mode on via IPC and show CC toggle active', async () => {
    await toggleClaudeCodeMode(workspaceId, true);

    const hasToggle = await elementExists('.claude-code-toggle.active');
    expect(hasToggle).toBe(true);
  });

  it('should reflect enabled state in the store', async () => {
    const mode = await getClaudeCodeMode(workspaceId);
    expect(mode).toBe(true);
  });

  it('should auto-execute claude command when creating a terminal with CC mode on', async () => {
    // CC mode is still on from prior test
    const countBefore = await getElementCount('.tab');
    await createNewTerminalTab();

    const countAfter = await getElementCount('.tab');
    expect(countAfter).toBe(countBefore + 1);

    // Wait for the auto-execute: the 500ms delay + shell init time
    // The command 'claude -dangerously-skip-permissions' should appear
    // in the terminal buffer (either as the command itself or its output/error)
    await waitForTerminalText('claude', 30000);
  });

  it('should toggle Claude Code mode off and deactivate CC toggle', async () => {
    await toggleClaudeCodeMode(workspaceId, false);

    const hasActiveToggle = await elementExists('.claude-code-toggle.active');
    expect(hasActiveToggle).toBe(false);

    // Toggle button should still exist, just not active
    const hasToggle = await elementExists('.claude-code-toggle');
    expect(hasToggle).toBe(true);
  });

  it('should NOT auto-execute claude command when CC mode is off', async () => {
    const countBefore = await getElementCount('.tab');
    await createNewTerminalTab();

    const countAfter = await getElementCount('.tab');
    expect(countAfter).toBe(countBefore + 1);

    // Wait for shell to initialize
    await browser.pause(3000);

    // Read the terminal grid text via Tauri IPC
    const text = await browser.executeAsync(async (done: (result: string) => void) => {
      try {
        const pane = document.querySelector('.terminal-pane.active') as any;
        const terminalId = pane?.getAttribute('data-terminal-id');
        if (!terminalId) { done(''); return; }
        const invoke = (window as any).__TAURI__?.core?.invoke;
        if (!invoke) { done(''); return; }
        const result = await invoke('get_grid_text', {
          terminalId, startRow: 0, startCol: 0, endRow: 999, endCol: 999,
        });
        done(result ?? '');
      } catch { done(''); }
    });

    // The 'claude -dangerously-skip-permissions' command should NOT be present
    expect(text).not.toContain('claude -dangerously-skip-permissions');
  });

  it('should persist Claude Code mode via toggle_claude_code_mode IPC', async () => {
    // Enable CC mode
    await toggleClaudeCodeMode(workspaceId, true);

    // Verify it's enabled in store
    const enabled = await getClaudeCodeMode(workspaceId);
    expect(enabled).toBe(true);

    // Disable it
    await toggleClaudeCodeMode(workspaceId, false);

    const disabled = await getClaudeCodeMode(workspaceId);
    expect(disabled).toBe(false);
  });
});
