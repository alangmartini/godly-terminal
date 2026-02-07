import {
  waitForAppReady,
  waitForTerminalPane,
  sendCommand,
  getElementCount,
  elementExists,
  clickElement,
  getElementClasses,
  createNewTerminalTab,
} from '../helpers/app';
import { getTerminalText, waitForTerminalText } from '../helpers/terminal-reader';

describe('Terminal I/O', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
  });

  it('should show a terminal pane and tab on launch', async () => {
    const hasPaneActive = await elementExists('.terminal-pane.active');
    expect(hasPaneActive).toBe(true);

    const tabCount = await getElementCount('.tab');
    expect(tabCount).toBeGreaterThanOrEqual(1);
  });

  it('should receive terminal output and echo commands', async () => {
    // Wait for the terminal to have some output (prompt or any data)
    // PowerShell may take several seconds to initialize
    await browser.pause(5000);

    const marker = 'E2E_TEST_OUTPUT_' + Date.now();
    await sendCommand(`echo "${marker}"`);
    await waitForTerminalText(marker, 30000);

    const text = await getTerminalText();
    expect(text).toContain(marker);
  });

  it('should create a second terminal via IPC', async () => {
    const countBefore = await getElementCount('.tab');
    await createNewTerminalTab();

    const countAfter = await getElementCount('.tab');
    expect(countAfter).toBe(countBefore + 1);
  });

  it('should switch terminals by clicking tabs', async () => {
    const tabCount = await getElementCount('.tab');
    expect(tabCount).toBeGreaterThanOrEqual(2);

    // Click the first tab
    await clickElement('.tab:first-child');
    await browser.pause(500);

    const firstTabClasses = await getElementClasses('.tab:first-child');
    expect(firstTabClasses).toContain('active');

    // Click the second tab
    await clickElement('.tab:nth-child(2)');
    await browser.pause(500);

    const secondTabClasses = await getElementClasses('.tab:nth-child(2)');
    expect(secondTabClasses).toContain('active');
  });

  it('should close a terminal via the tab close button', async () => {
    const countBefore = await getElementCount('.tab');

    // Click the close button on the last tab
    await clickElement('.tab:last-child .tab-close');

    await browser.waitUntil(
      async () => {
        const count = await getElementCount('.tab');
        return count < countBefore;
      },
      { timeout: 10000, timeoutMsg: 'Tab was not closed' }
    );

    const countAfter = await getElementCount('.tab');
    expect(countAfter).toBe(countBefore - 1);
  });
});
