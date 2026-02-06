import { waitForAppReady, sendCommand } from '../helpers/app';
import { getTerminalText, waitForTerminalText } from '../helpers/terminal-reader';

describe('Terminal I/O', () => {
  before(async () => {
    await waitForAppReady();
  });

  it('should show a terminal pane and tab on launch', async () => {
    const pane = await browser.$('.terminal-pane.active');
    expect(await pane.isExisting()).toBe(true);

    const tabs = await browser.$$('.tab');
    expect(tabs.length).toBeGreaterThanOrEqual(1);
  });

  it('should show a PowerShell prompt', async () => {
    // PowerShell prompts typically contain "PS " or ">"
    await waitForTerminalText('PS ', 30000);
    const text = await getTerminalText();
    expect(text).toContain('PS ');
  });

  it('should echo typed commands in the terminal buffer', async () => {
    const marker = 'E2E_TEST_OUTPUT_' + Date.now();
    await sendCommand(`echo "${marker}"`);
    await waitForTerminalText(marker, 15000);

    const text = await getTerminalText();
    expect(text).toContain(marker);
  });

  it('should create a second terminal via the add-tab button', async () => {
    const tabsBefore = await browser.$$('.tab');
    const countBefore = tabsBefore.length;

    const addBtn = await browser.$('.add-tab-btn');
    await addBtn.click();

    // Wait for the new tab to appear
    await browser.waitUntil(
      async () => {
        const tabs = await browser.$$('.tab');
        return tabs.length > countBefore;
      },
      { timeout: 15000, timeoutMsg: 'New tab did not appear' }
    );

    const tabsAfter = await browser.$$('.tab');
    expect(tabsAfter.length).toBe(countBefore + 1);
  });

  it('should switch terminals by clicking tabs', async () => {
    const tabs = await browser.$$('.tab');
    expect(tabs.length).toBeGreaterThanOrEqual(2);

    // Click the first tab
    await tabs[0].click();
    await browser.pause(500);

    // Verify first tab is active
    const firstTabClass = await tabs[0].getAttribute('class');
    expect(firstTabClass).toContain('active');

    // Click the second tab
    await tabs[1].click();
    await browser.pause(500);

    const secondTabClass = await tabs[1].getAttribute('class');
    expect(secondTabClass).toContain('active');
  });

  it('should close a terminal via the tab close button', async () => {
    const tabsBefore = await browser.$$('.tab');
    const countBefore = tabsBefore.length;

    // Close the last tab
    const lastTab = tabsBefore[countBefore - 1];
    const closeBtn = await lastTab.$('.tab-close');
    await closeBtn.click();

    await browser.waitUntil(
      async () => {
        const tabs = await browser.$$('.tab');
        return tabs.length < countBefore;
      },
      { timeout: 10000, timeoutMsg: 'Tab was not closed' }
    );

    const tabsAfter = await browser.$$('.tab');
    expect(tabsAfter.length).toBe(countBefore - 1);
  });
});
