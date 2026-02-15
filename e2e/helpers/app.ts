/**
 * General app-level helpers for E2E tests.
 *
 * Uses browser.execute() for all DOM queries since WebView2's WebDriver
 * implementation doesn't reliably find dynamically-created elements via
 * browser.$() / browser.$$().
 *
 * Uses browser.executeAsync() + Tauri IPC for terminal I/O since
 * browser.keys() doesn't reliably reach the terminal canvas in WebView2.
 */

/**
 * Wait until the app's main UI has loaded — either a terminal pane or the
 * empty-state placeholder is present in the DOM.
 */
export async function waitForAppReady(timeout = 60000): Promise<void> {
  // Initial pause to let the Tauri app load its webview content
  await browser.pause(3000);

  await browser.waitUntil(
    async () => {
      try {
        return await browser.execute(() => {
          return !!(
            document.querySelector('.terminal-pane') ||
            document.querySelector('.empty-state') ||
            document.querySelector('.sidebar')
          );
        });
      } catch {
        return false;
      }
    },
    {
      timeout,
      interval: 1000,
      timeoutMsg: 'App did not become ready within timeout',
    }
  );
}

/**
 * Wait for a terminal pane to be active and visible.
 */
export async function waitForTerminalPane(timeout = 30000): Promise<void> {
  await browser.waitUntil(
    async () => {
      try {
        return await browser.execute(() => {
          return !!document.querySelector('.terminal-pane.active');
        });
      } catch {
        return false;
      }
    },
    {
      timeout,
      interval: 1000,
      timeoutMsg: 'No active terminal pane found',
    }
  );
}

/**
 * Write text to the active terminal via Tauri IPC (more reliable than browser.keys).
 * Uses fire-and-forget: invoke runs asynchronously in the browser, we don't await it.
 */
export async function typeInTerminal(text: string): Promise<void> {
  const status = await browser.execute((txt: string) => {
    const pane = document.querySelector('.terminal-pane.active');
    const terminalId = pane?.getAttribute('data-terminal-id');
    if (!terminalId) return 'error: no active terminal pane';
    const invoke = (window as any).__TAURI__?.core?.invoke;
    if (!invoke) return 'error: __TAURI__ not available';
    // Fire and forget — the invoke promise runs in the background
    invoke('write_to_terminal', { terminalId, data: txt }).catch((e: any) => {
      console.error('[e2e] write_to_terminal failed:', e);
    });
    return 'ok';
  }, text);
  if (typeof status === 'string' && status.startsWith('error:')) {
    throw new Error(`typeInTerminal failed: ${status}`);
  }
  // Small pause to let the IPC complete
  await browser.pause(300);
}

/**
 * Send a command to the terminal (text + carriage return) via Tauri IPC.
 */
export async function sendCommand(command: string): Promise<void> {
  await typeInTerminal(command + '\r');
}

/**
 * Get element count for a CSS selector via browser.execute().
 */
export async function getElementCount(selector: string): Promise<number> {
  return browser.execute((sel) => {
    return document.querySelectorAll(sel).length;
  }, selector);
}

/**
 * Check if an element matching the selector exists.
 */
export async function elementExists(selector: string): Promise<boolean> {
  return browser.execute((sel) => {
    return !!document.querySelector(sel);
  }, selector);
}

/**
 * Click an element by CSS selector via browser.execute().
 */
export async function clickElement(selector: string): Promise<void> {
  await browser.execute((sel) => {
    const el = document.querySelector(sel) as HTMLElement;
    if (el) el.click();
  }, selector);
}

/**
 * Get an attribute of an element by selector.
 */
export async function getElementAttribute(
  selector: string,
  attr: string
): Promise<string | null> {
  return browser.execute(
    (sel, attribute) => {
      const el = document.querySelector(sel);
      return el ? el.getAttribute(attribute) : null;
    },
    selector,
    attr
  );
}

/**
 * Get the class list of an element by selector.
 */
export async function getElementClasses(selector: string): Promise<string> {
  return browser.execute((sel) => {
    const el = document.querySelector(sel);
    return el ? el.className : '';
  }, selector);
}

/**
 * Get the active terminal's ID from the DOM.
 */
export async function getActiveTerminalId(): Promise<string | null> {
  return browser.execute(() => {
    const pane = document.querySelector('.terminal-pane.active');
    return pane?.getAttribute('data-terminal-id') ?? null;
  });
}

/**
 * Create a new terminal tab via Tauri IPC + store.
 * Uses the exposed __store and __TAURI__ globals.
 * Fires the IPC call asynchronously and waits for the tab to appear.
 */
export async function createNewTerminalTab(): Promise<void> {
  const countBefore = await getElementCount('.tab');

  // Fire the create_terminal IPC call asynchronously
  const status = await browser.execute(() => {
    const invoke = (window as any).__TAURI__?.core?.invoke;
    const appStore = (window as any).__store;
    if (!invoke) return 'error: __TAURI__ not available';
    if (!appStore) return 'error: __store not available';

    const state = appStore.getState();
    const workspaceId = state.activeWorkspaceId;
    if (!workspaceId) return 'error: no active workspace';

    // Fire and forget — IPC runs in background, then updates store
    invoke('create_terminal', {
      workspaceId,
      cwdOverride: null,
      shellTypeOverride: null,
      idOverride: null,
    })
      .then((terminalId: string) => {
        appStore.addTerminal({
          id: terminalId,
          workspaceId,
          name: 'Terminal',
          processName: 'powershell',
          order: 0,
        });
      })
      .catch((e: any) => {
        console.error('[e2e] create_terminal failed:', e);
      });

    return 'ok';
  });

  if (typeof status === 'string' && status.startsWith('error:')) {
    throw new Error(`createNewTerminalTab failed: ${status}`);
  }

  // Wait for the tab count to increase
  await browser.waitUntil(
    async () => (await getElementCount('.tab')) > countBefore,
    { timeout: 15000, timeoutMsg: 'New tab did not appear in DOM' }
  );
}

/**
 * Trigger a layout save via the Tauri IPC API inside the webview.
 */
export async function triggerSave(): Promise<void> {
  const status = await browser.execute(() => {
    const invoke = (window as any).__TAURI__?.core?.invoke;
    if (!invoke) return 'no-tauri';
    invoke('save_layout').catch((e: any) => {
      console.error('[e2e] save_layout failed:', e);
    });
    return 'ok';
  });

  if (status === 'no-tauri') {
    await browser.keys(['Control', 'Shift', 's']);
  }
  // Wait for the save to complete on disk
  await browser.pause(2000);
}
