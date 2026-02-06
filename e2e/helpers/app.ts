/**
 * General app-level helpers for E2E tests.
 */

/**
 * Wait until the app's main UI has loaded — either a terminal pane or the
 * empty-state placeholder is present in the DOM.
 */
export async function waitForAppReady(timeout = 30000): Promise<void> {
  await browser.waitUntil(
    async () => {
      const pane = await browser.$('.terminal-pane');
      const empty = await browser.$('.empty-state');
      return pane.isExisting() || empty.isExisting();
    },
    { timeout, timeoutMsg: 'App did not become ready within timeout' }
  );
}

/**
 * Click the active terminal pane to ensure it has focus, then send keys.
 */
export async function typeInTerminal(text: string): Promise<void> {
  const pane = await browser.$('.terminal-pane.active');
  await pane.click();
  await browser.keys(text.split(''));
}

/**
 * Send a command to the terminal (type + Enter).
 */
export async function sendCommand(command: string): Promise<void> {
  await typeInTerminal(command);
  await browser.keys(['Enter']);
}

/**
 * Trigger a layout save via the Tauri IPC API inside the webview.
 * Falls back to keyboard shortcut if __TAURI__ is not available.
 */
export async function triggerSave(): Promise<void> {
  const saved = await browser.execute(async () => {
    try {
      const { invoke } = (window as any).__TAURI__.core;
      await invoke('save_layout');
      return true;
    } catch {
      return false;
    }
  });

  if (!saved) {
    // Fallback: Ctrl+Shift+S
    await browser.keys(['Control', 'Shift', 's']);
    // Give it a moment to persist
    await browser.pause(2000);
  }
}
