/**
 * E2E test for Bug #405: Split pane containers survive new tab creation.
 *
 * Reproduces the full user scenario:
 * 1. Split right (Ctrl+\)
 * 2. Create new tab (Ctrl+T) — internally destroys split
 * 3. Switch back to original tab — pane should be visible with content
 *
 * The browser test (SplitContainer.lifecycle.browser.test.ts) covers the
 * DOM-level fix; this exercises the full Tauri app with real keyboard input,
 * tab switching, and terminal rendering.
 */
import {
  waitForAppReady,
  waitForTerminalPane,
  getElementCount,
  elementExists,
  clickElement,
} from '../helpers/app';
import { getTerminalTextAsync } from '../helpers/terminal-reader';

describe('Bug #405: split pane survives new tab creation', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
    // Wait for shell to initialize fully
    await browser.pause(5000);
  });

  it('split → new tab → switch back: original pane is visible', async () => {
    // Record initial state
    const tabCountBefore = await getElementCount('.tab');
    const paneCountBefore = await getElementCount('.terminal-pane');

    // Step 1: Split right (Ctrl+\)
    await browser.keys(['Control', '\\']);
    await browser.pause(2000);

    // Verify split created — should have 2 visible panes
    const splitPaneCount = await getElementCount('.terminal-pane.split-visible');
    expect(splitPaneCount).toBe(2);

    // Verify split-root exists in DOM
    const hasSplitRoot = await elementExists('.split-root');
    expect(hasSplitRoot).toBe(true);

    // Step 2: Create new tab (Ctrl+T) — this destroys the split
    await browser.keys(['Control', 't']);
    await browser.waitUntil(
      async () => (await getElementCount('.tab')) > tabCountBefore,
      { timeout: 15000, timeoutMsg: 'Ctrl+T did not create a new tab' }
    );
    await browser.pause(1000);

    // Split should be gone now
    const hasSplitRootAfter = await elementExists('.split-root');
    expect(hasSplitRootAfter).toBe(false);

    // Step 3: Switch back to the first tab
    await clickElement('.tab:first-child');
    await browser.pause(1000);

    // The first tab should be active
    const firstTabActive = await browser.execute(() => {
      const firstTab = document.querySelector('.tab:first-child');
      return firstTab?.classList.contains('active') ?? false;
    });
    expect(firstTabActive).toBe(true);

    // Step 4: Assert the pane is visible in DOM with non-zero dimensions
    const hasActivePane = await elementExists('.terminal-pane.active');
    expect(hasActivePane).toBe(true);

    const paneRect = await browser.execute(() => {
      const pane = document.querySelector('.terminal-pane.active') as HTMLElement;
      if (!pane) return { width: 0, height: 0 };
      const rect = pane.getBoundingClientRect();
      return { width: rect.width, height: rect.height };
    });
    expect(paneRect.width).toBeGreaterThan(0);
    expect(paneRect.height).toBeGreaterThan(0);

    // Step 5: Assert the terminal has content (shell prompt or output)
    const text = await getTerminalTextAsync();
    expect(text.trim().length).toBeGreaterThan(0);
  });

  it('split → new tab → switch back to second split pane: pane is visible', async () => {
    // Start fresh — click the first tab to ensure we're on it
    await clickElement('.tab:first-child');
    await browser.pause(1000);

    // Step 1: Split right
    await browser.keys(['Control', '\\']);
    await browser.pause(2000);

    // Get the terminal ID of the second split pane
    const secondPaneId = await browser.execute(() => {
      const panes = document.querySelectorAll('.terminal-pane.split-visible');
      if (panes.length < 2) return null;
      return panes[1].getAttribute('data-terminal-id');
    });

    // Step 2: Create a new tab (destroys split)
    const tabCountBefore = await getElementCount('.tab');
    await browser.keys(['Control', 't']);
    await browser.waitUntil(
      async () => (await getElementCount('.tab')) > tabCountBefore,
      { timeout: 15000, timeoutMsg: 'Ctrl+T did not create a new tab' }
    );
    await browser.pause(1000);

    // Step 3: Click the tab that was the second split pane
    if (secondPaneId) {
      await browser.execute((id: string) => {
        const tab = document.querySelector(`.tab[data-terminal-id="${id}"]`) as HTMLElement;
        if (tab) tab.click();
      }, secondPaneId);
      await browser.pause(1000);

      // Assert the pane is visible
      const hasActivePane = await elementExists('.terminal-pane.active');
      expect(hasActivePane).toBe(true);

      const paneRect = await browser.execute(() => {
        const pane = document.querySelector('.terminal-pane.active') as HTMLElement;
        if (!pane) return { width: 0, height: 0 };
        const rect = pane.getBoundingClientRect();
        return { width: rect.width, height: rect.height };
      });
      expect(paneRect.width).toBeGreaterThan(0);
      expect(paneRect.height).toBeGreaterThan(0);
    }
  });

  after(async () => {
    // Clean up: close extra tabs to leave the app in a clean state
    // Close tabs down to 1 using Ctrl+W
    let tabCount = await getElementCount('.tab');
    while (tabCount > 1) {
      await browser.keys(['Control', 'w']);
      await browser.waitUntil(
        async () => (await getElementCount('.tab')) < tabCount,
        { timeout: 10000, timeoutMsg: 'Failed to close tab during cleanup' }
      );
      tabCount = await getElementCount('.tab');
    }
  });
});
