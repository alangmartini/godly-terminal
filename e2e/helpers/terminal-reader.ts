/**
 * Helpers to read xterm.js terminal buffer content via WebDriver.
 *
 * Relies on __xterm being exposed on the .terminal-pane container element
 * (see TerminalPane.ts mount()).
 */

/**
 * Read all text currently in the active terminal's xterm.js buffer.
 */
export async function getTerminalText(): Promise<string> {
  return browser.execute(() => {
    const pane = document.querySelector('.terminal-pane.active') as any;
    if (!pane?.__xterm) return '';
    const term = pane.__xterm;
    const buf = term.buffer.active;
    const lines: string[] = [];
    for (let i = 0; i < buf.length; i++) {
      const line = buf.getLine(i);
      if (line) lines.push(line.translateToString(true));
    }
    return lines.join('\n');
  });
}

/**
 * Poll the terminal buffer until `substring` appears, or timeout.
 */
export async function waitForTerminalText(
  substring: string,
  timeout = 30000
): Promise<string> {
  const start = Date.now();
  let lastText = '';
  while (Date.now() - start < timeout) {
    lastText = await getTerminalText();
    if (lastText.includes(substring)) return lastText;
    await browser.pause(500);
  }
  throw new Error(
    `Terminal text did not contain "${substring}" within ${timeout}ms.\n` +
    `Last buffer content (last 500 chars):\n${lastText.slice(-500)}`
  );
}
