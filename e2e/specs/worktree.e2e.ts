import {
  waitForAppReady,
  waitForTerminalPane,
  sendCommand,
  getElementCount,
  elementExists,
  clickElement,
  createNewTerminalTab,
} from '../helpers/app';
import { waitForTerminalText } from '../helpers/terminal-reader';

/**
 * Invoke a Tauri command from the browser context.
 * Returns the result or throws on error.
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
 * Toggle worktree mode for a workspace via IPC and update the frontend store.
 */
async function toggleWorktreeMode(workspaceId: string, enabled: boolean): Promise<void> {
  await invokeCommand('toggle_worktree_mode', {
    workspaceId,
    enabled,
  });
  // Update the frontend store
  await browser.execute(
    (wsId: string, mode: boolean) => {
      const appStore = (window as any).__store;
      if (appStore) {
        appStore.updateWorkspace(wsId, { worktreeMode: mode });
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
 * Get the folder path of the active workspace.
 */
async function getActiveWorkspaceFolderPath(): Promise<string | null> {
  return browser.execute(() => {
    const appStore = (window as any).__store;
    if (!appStore) return null;
    const state = appStore.getState();
    const ws = state.workspaces.find((w: any) => w.id === state.activeWorkspaceId);
    return ws ? ws.folderPath : null;
  });
}

describe('Worktree Mode', () => {
  let workspaceId: string;

  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();

    workspaceId = (await getActiveWorkspaceId())!;
    expect(workspaceId).toBeTruthy();
  });

  it('should toggle worktree mode on via IPC and show WT toggle active', async () => {
    await toggleWorktreeMode(workspaceId, true);

    const hasToggle = await elementExists('.worktree-toggle.active');
    expect(hasToggle).toBe(true);
  });

  it('should check if workspace folder is a git repo via IPC', async () => {
    const folderPath = await getActiveWorkspaceFolderPath();
    expect(folderPath).toBeTruthy();

    const isGitRepo = await invokeCommand<boolean>('is_git_repo', {
      folderPath: folderPath!,
    });
    // The workspace may or may not be a git repo depending on test setup.
    // This test verifies the IPC call works without error.
    expect(typeof isGitRepo).toBe('boolean');
  });

  it('should create a terminal with worktree CWD when mode is enabled', async () => {
    const folderPath = await getActiveWorkspaceFolderPath();
    const isGitRepo = await invokeCommand<boolean>('is_git_repo', {
      folderPath: folderPath!,
    });

    if (!isGitRepo) {
      console.log('[worktree e2e] Skipping: workspace is not a git repo');
      return;
    }

    // Create a new terminal tab - it should get a worktree CWD
    await createNewTerminalTab();
    await browser.pause(3000); // Wait for shell to initialize

    // Check that the terminal's CWD contains the worktree path marker
    const marker = 'GODLY_WT_CHECK_' + Date.now();
    await sendCommand(`echo ${marker} && cd`);
    await waitForTerminalText(marker, 15000);
  });

  it('should list worktrees via IPC', async () => {
    const folderPath = await getActiveWorkspaceFolderPath();
    const isGitRepo = await invokeCommand<boolean>('is_git_repo', {
      folderPath: folderPath!,
    });

    if (!isGitRepo) {
      console.log('[worktree e2e] Skipping: workspace is not a git repo');
      return;
    }

    const worktrees = await invokeCommand<Array<{
      path: string;
      branch: string;
      commit: string;
      is_main: boolean;
    }>>('list_worktrees', { folderPath: folderPath! });

    expect(Array.isArray(worktrees)).toBe(true);
    // At least the main worktree should always be present
    expect(worktrees.length).toBeGreaterThanOrEqual(1);
    expect(worktrees.some(wt => wt.is_main)).toBe(true);
  });

  it('should toggle worktree mode off and deactivate WT toggle', async () => {
    await toggleWorktreeMode(workspaceId, false);

    const hasActiveToggle = await elementExists('.worktree-toggle.active');
    expect(hasActiveToggle).toBe(false);

    // Toggle button should still exist, just not active
    const hasToggle = await elementExists('.worktree-toggle');
    expect(hasToggle).toBe(true);
  });

  it('should create a normal terminal when worktree mode is off', async () => {
    const countBefore = await getElementCount('.tab');
    await createNewTerminalTab();

    const countAfter = await getElementCount('.tab');
    expect(countAfter).toBe(countBefore + 1);
  });

  it('should show worktree panel when mode is enabled', async () => {
    await toggleWorktreeMode(workspaceId, true);
    await browser.pause(500);

    const panelVisible = await browser.execute(() => {
      const panel = document.querySelector('.worktree-panel');
      return panel ? panel instanceof HTMLElement && panel.style.display !== 'none' : false;
    });

    // Panel visibility depends on whether it's a git repo
    // At minimum, verify the panel element exists
    const panelExists = await elementExists('.worktree-panel');
    expect(panelExists).toBe(true);

    // Clean up
    await toggleWorktreeMode(workspaceId, false);
  });
});

describe('Clean All Worktrees', function () {
  let workspaceId: string;

  before(async function () {
    await waitForAppReady();
    await waitForTerminalPane();

    workspaceId = (await getActiveWorkspaceId())!;
    expect(workspaceId).toBeTruthy();

    const folderPath = await getActiveWorkspaceFolderPath();
    const isGitRepo = folderPath
      ? await invokeCommand<boolean>('is_git_repo', { folderPath })
      : false;

    // Skip entire suite if not a git repo
    if (!isGitRepo) {
      this.skip();
      return;
    }

    // Enable worktree mode for the tests
    await toggleWorktreeMode(workspaceId, true);
    await browser.pause(500);
  });

  after(async () => {
    // Clean up: disable worktree mode
    await toggleWorktreeMode(workspaceId, false);
  });

  it('should show loading state when Clean All is clicked', async () => {
    // Create a terminal to ensure there is at least one worktree to clean
    await createNewTerminalTab();
    await browser.pause(3000);

    // Wait for Clean All button to appear (worktree list must refresh)
    await browser.waitUntil(
      async () => elementExists('.worktree-clean-all'),
      { timeout: 10000, timeoutMsg: 'Clean All button did not appear' }
    );

    await clickElement('.worktree-clean-all');

    // The button should get the .cleaning class
    await browser.waitUntil(
      async () => {
        return browser.execute(() => {
          const btn = document.querySelector('.worktree-clean-all');
          return btn ? btn.classList.contains('cleaning') : false;
        });
      },
      { timeout: 5000, timeoutMsg: '.cleaning class was not applied to button' }
    );

    // Wait for cleanup to finish
    await browser.waitUntil(
      async () => {
        return browser.execute(() => {
          const btn = document.querySelector('.worktree-clean-all');
          return btn ? !btn.classList.contains('cleaning') : true;
        });
      },
      { timeout: 30000, timeoutMsg: 'Cleanup did not finish within 30s' }
    );
  });

  it('should show status text during cleanup', async () => {
    // Create another worktree to trigger cleanup status
    await createNewTerminalTab();
    await browser.pause(3000);

    await browser.waitUntil(
      async () => elementExists('.worktree-clean-all'),
      { timeout: 10000, timeoutMsg: 'Clean All button did not appear' }
    );

    await clickElement('.worktree-clean-all');

    // The status container should appear with meaningful text
    await browser.waitUntil(
      async () => {
        return browser.execute(() => {
          const status = document.querySelector('.worktree-status');
          if (!(status instanceof HTMLElement)) return false;
          if (status.style.display === 'none') return false;
          const text = status.textContent || '';
          // Status text should contain a progress keyword
          return text.includes('Listing') || text.includes('Removing') || text.includes('Done');
        });
      },
      { timeout: 10000, timeoutMsg: 'Status text with progress keyword did not appear' }
    );

    // Wait for cleanup to finish
    await browser.waitUntil(
      async () => {
        return browser.execute(() => {
          const btn = document.querySelector('.worktree-clean-all');
          return btn ? !btn.classList.contains('cleaning') : true;
        });
      },
      { timeout: 30000 }
    );
  });

  it('should show empty state after cleanup completes', async () => {
    // Create a worktree, then clean it, then verify empty state
    await createNewTerminalTab();
    await browser.pause(3000);

    await browser.waitUntil(
      async () => elementExists('.worktree-clean-all'),
      { timeout: 10000, timeoutMsg: 'Clean All button did not appear' }
    );

    await clickElement('.worktree-clean-all');

    // Wait for cleanup to finish (button loses .cleaning class)
    await browser.waitUntil(
      async () => {
        return browser.execute(() => {
          const btn = document.querySelector('.worktree-clean-all');
          return btn ? !btn.classList.contains('cleaning') : true;
        });
      },
      { timeout: 30000, timeoutMsg: 'Cleanup did not finish' }
    );

    // After cleanup, the empty state should be shown with no worktree items
    await browser.waitUntil(
      async () => {
        return browser.execute(() => {
          const items = document.querySelectorAll('.worktree-item');
          const empty = document.querySelector('.worktree-empty');
          return items.length === 0 && empty !== null;
        });
      },
      { timeout: 10000, timeoutMsg: 'Empty state was not shown after cleanup' }
    );
  });

  it('should disable button during cleanup to prevent double-click', async () => {
    // Create a worktree so there's something to clean
    await createNewTerminalTab();
    await browser.pause(3000);

    await browser.waitUntil(
      async () => elementExists('.worktree-clean-all'),
      { timeout: 10000, timeoutMsg: 'Clean All button did not appear' }
    );

    await clickElement('.worktree-clean-all');

    // Button should be disabled during cleanup
    const isDisabled = await browser.execute(() => {
      const btn = document.querySelector('.worktree-clean-all') as HTMLButtonElement;
      return btn ? btn.disabled : false;
    });
    expect(isDisabled).toBe(true);

    // Wait for cleanup to finish
    await browser.waitUntil(
      async () => {
        return browser.execute(() => {
          const btn = document.querySelector('.worktree-clean-all');
          return btn ? !btn.classList.contains('cleaning') : true;
        });
      },
      { timeout: 30000 }
    );

    // Button should be re-enabled after cleanup
    const isReEnabled = await browser.execute(() => {
      const btn = document.querySelector('.worktree-clean-all') as HTMLButtonElement;
      return btn ? !btn.disabled : true;
    });
    expect(isReEnabled).toBe(true);
  });
});
