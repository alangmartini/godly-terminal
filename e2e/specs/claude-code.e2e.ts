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
 * Set AI tool mode for a workspace via IPC and update the frontend store.
 */
async function setAiToolMode(workspaceId: string, mode: string): Promise<void> {
  await invokeCommand('set_ai_tool_mode', {
    workspaceId,
    mode,
  });
  await browser.execute(
    (wsId: string, aiMode: string) => {
      const appStore = (window as any).__store;
      if (appStore) {
        appStore.updateWorkspace(wsId, { aiToolMode: aiMode });
      }
    },
    workspaceId,
    mode
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
 * Get the aiToolMode flag for a workspace from the store.
 */
async function getAiToolMode(workspaceId: string): Promise<string> {
  return browser.execute((wsId: string) => {
    const appStore = (window as any).__store;
    if (!appStore) return 'none';
    const ws = appStore.getState().workspaces.find((w: any) => w.id === wsId);
    return ws ? ws.aiToolMode : 'none';
  }, workspaceId);
}

describe('AI Tool Mode (Claude Code)', () => {
  let workspaceId: string;

  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();

    workspaceId = (await getActiveWorkspaceId())!;
    expect(workspaceId).toBeTruthy();
  });

  after(async () => {
    // Ensure AI tool mode is off after tests
    await setAiToolMode(workspaceId, 'none');
  });

  it('should set AI tool mode to claude via IPC and show AI tool toggle active', async () => {
    await setAiToolMode(workspaceId, 'claude');

    const hasToggle = await elementExists('.ai-tool-toggle.active');
    expect(hasToggle).toBe(true);
  });

  it('should reflect enabled state in the store', async () => {
    const mode = await getAiToolMode(workspaceId);
    expect(mode).toBe('claude');
  });

  it('should auto-execute claude command when creating a terminal with AI tool mode claude', async () => {
    // AI tool mode is still claude from prior test
    const countBefore = await getElementCount('.tab');
    await createNewTerminalTab();

    const countAfter = await getElementCount('.tab');
    expect(countAfter).toBe(countBefore + 1);

    // Wait for the auto-execute: the 500ms delay + shell init time
    // The command 'claude --dangerously-skip-permissions' should appear
    // in the terminal buffer (either as the command itself or its output/error)
    await waitForTerminalText('claude', 30000);
  });

  it('should set AI tool mode to none and deactivate AI tool toggle', async () => {
    await setAiToolMode(workspaceId, 'none');

    const hasActiveToggle = await elementExists('.ai-tool-toggle.active');
    expect(hasActiveToggle).toBe(false);

    // Toggle button should still exist, just not active
    const hasToggle = await elementExists('.ai-tool-toggle');
    expect(hasToggle).toBe(true);
  });

  it('should NOT auto-execute claude command when AI tool mode is none', async () => {
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

    // The 'claude --dangerously-skip-permissions' command should NOT be present
    expect(text).not.toContain('claude --dangerously-skip-permissions');
  });

  it('should persist AI tool mode via set_ai_tool_mode IPC', async () => {
    // Enable claude mode
    await setAiToolMode(workspaceId, 'claude');

    // Verify it's enabled in store
    const enabled = await getAiToolMode(workspaceId);
    expect(enabled).toBe('claude');

    // Disable it
    await setAiToolMode(workspaceId, 'none');

    const disabled = await getAiToolMode(workspaceId);
    expect(disabled).toBe('none');
  });
});
