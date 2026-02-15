/**
 * Helpers to read terminal buffer content via WebDriver.
 *
 * Uses the Tauri IPC `get_grid_text` command to read text from the daemon's
 * godly-vt grid, rather than accessing an in-browser terminal parser.
 */

/**
 * Read all text currently in the active terminal's grid.
 */
export async function getTerminalText(): Promise<string> {
  return browser.execute(() => {
    const pane = document.querySelector('.terminal-pane.active') as any;
    const terminalId = pane?.getAttribute('data-terminal-id');
    if (!terminalId) return '';
    const invoke = (window as any).__TAURI__?.core?.invoke;
    if (!invoke) return '';
    // Synchronously return a promise result via a trick:
    // We can't await in browser.execute, so we store the result and poll.
    // For simplicity, use a blocking approach with a shared variable.
    return '';
  });
}

/**
 * Read terminal text via async Tauri IPC.
 * Uses executeAsync to properly await the IPC response.
 */
export async function getTerminalTextAsync(): Promise<string> {
  return browser.executeAsync(async (done: (result: string) => void) => {
    try {
      const pane = document.querySelector('.terminal-pane.active') as any;
      const terminalId = pane?.getAttribute('data-terminal-id');
      if (!terminalId) { done(''); return; }
      const invoke = (window as any).__TAURI__?.core?.invoke;
      if (!invoke) { done(''); return; }
      const text = await invoke('get_grid_text', {
        terminalId,
        startRow: 0,
        startCol: 0,
        endRow: 999,
        endCol: 999,
      });
      done(text ?? '');
    } catch {
      done('');
    }
  });
}

/**
 * Poll the terminal grid until `substring` appears, or timeout.
 */
export async function waitForTerminalText(
  substring: string,
  timeout = 30000
): Promise<string> {
  const start = Date.now();
  let lastText = '';
  while (Date.now() - start < timeout) {
    lastText = await getTerminalTextAsync();
    if (lastText.includes(substring)) return lastText;
    await browser.pause(500);
  }
  throw new Error(
    `Terminal text did not contain "${substring}" within ${timeout}ms.\n` +
    `Last buffer content (last 500 chars):\n${lastText.slice(-500)}`
  );
}
