import {
  waitForAppReady,
  waitForTerminalPane,
  getElementCount,
  elementExists,
  createNewTerminalTab,
} from '../helpers/app';

/**
 * Double-click the active tab's title to start rename mode.
 * Returns true if the rename input appeared.
 */
async function startRenameByDoubleClick(): Promise<boolean> {
  return browser.execute(() => {
    const activeTab = document.querySelector('.tab.active .tab-title') as HTMLElement;
    if (!activeTab) return false;
    // Simulate dblclick event
    const event = new MouseEvent('dblclick', { bubbles: true });
    activeTab.dispatchEvent(event);
    return true;
  });
}

/**
 * Wait for the inline rename input to appear on the active tab.
 */
async function waitForRenameInput(timeout = 5000): Promise<void> {
  await browser.waitUntil(
    async () => elementExists('.tab.active input.tab-title.editing'),
    { timeout, timeoutMsg: 'Rename input did not appear' }
  );
}

/**
 * Type text into the rename input and confirm with Enter.
 */
async function confirmRename(newName: string): Promise<void> {
  await browser.execute((name: string) => {
    const input = document.querySelector('.tab.active input.tab-title.editing') as HTMLInputElement;
    if (input) {
      input.value = name;
      // Dispatch input event so any listeners fire
      input.dispatchEvent(new Event('input', { bubbles: true }));
      // Confirm with Enter
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
    }
  }, newName);
}

/**
 * Cancel the current rename by pressing Escape.
 */
async function cancelRename(): Promise<void> {
  await browser.execute(() => {
    const input = document.querySelector('.tab.active input.tab-title.editing') as HTMLInputElement;
    if (input) {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    }
  });
}

/**
 * Get the displayed text of the active tab's title.
 */
async function getActiveTabTitle(): Promise<string> {
  return browser.execute(() => {
    const title = document.querySelector('.tab.active .tab-title');
    return title?.textContent?.trim() ?? '';
  });
}

/**
 * Open context menu on the active tab via right-click event.
 */
async function openTabContextMenu(): Promise<void> {
  await browser.execute(() => {
    const tab = document.querySelector('.tab.active') as HTMLElement;
    if (tab) {
      const event = new MouseEvent('contextmenu', {
        bubbles: true,
        clientX: tab.getBoundingClientRect().x + 10,
        clientY: tab.getBoundingClientRect().y + 10,
      });
      tab.dispatchEvent(event);
    }
  });
}

describe('Terminal Rename', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
    // Wait for shell to initialize
    await browser.pause(5000);
  });

  describe('Double-click rename', () => {
    it('should enter rename mode on double-click', async () => {
      const triggered = await startRenameByDoubleClick();
      expect(triggered).toBe(true);

      await waitForRenameInput();

      const inputExists = await elementExists('.tab.active input.tab-title.editing');
      expect(inputExists).toBe(true);
    });

    it('should confirm rename with Enter', async () => {
      // If not already in rename mode, start it
      if (!(await elementExists('.tab.active input.tab-title.editing'))) {
        await startRenameByDoubleClick();
        await waitForRenameInput();
      }

      const newName = 'Renamed-Tab-' + Date.now();
      await confirmRename(newName);

      // Wait for the input to be replaced by the title span
      await browser.waitUntil(
        async () => !(await elementExists('.tab.active input.tab-title.editing')),
        { timeout: 5000, timeoutMsg: 'Rename input did not disappear after Enter' }
      );

      const title = await getActiveTabTitle();
      expect(title).toBe(newName);
    });

    it('should persist the renamed name in the store', async () => {
      const storeTerminal = await browser.execute(() => {
        const store = (window as any).__store;
        if (!store) return null;
        const state = store.getState();
        const activeId = state.activeTerminalId;
        return state.terminals.find((t: any) => t.id === activeId) ?? null;
      });

      expect(storeTerminal).not.toBeNull();
      expect(storeTerminal.name).toContain('Renamed-Tab-');
    });
  });

  describe('Cancel rename with Escape', () => {
    it('should start rename and cancel with Escape', async () => {
      const titleBefore = await getActiveTabTitle();

      await startRenameByDoubleClick();
      await waitForRenameInput();

      // Type something different in the input
      await browser.execute(() => {
        const input = document.querySelector('.tab.active input.tab-title.editing') as HTMLInputElement;
        if (input) input.value = 'SHOULD_NOT_PERSIST';
      });

      await cancelRename();

      // Wait for rename mode to exit
      await browser.waitUntil(
        async () => !(await elementExists('.tab.active input.tab-title.editing')),
        { timeout: 5000, timeoutMsg: 'Rename input did not disappear after Escape' }
      );

      const titleAfter = await getActiveTabTitle();
      expect(titleAfter).toBe(titleBefore);
    });
  });

  describe('Context menu rename', () => {
    it('should show a context menu with Rename option on right-click', async () => {
      await openTabContextMenu();
      await browser.pause(300);

      const menuExists = await elementExists('.context-menu');
      expect(menuExists).toBe(true);

      // Check that Rename is an option
      const hasRename = await browser.execute(() => {
        const items = document.querySelectorAll('.context-menu-item');
        return Array.from(items).some((item) => item.textContent?.trim() === 'Rename');
      });
      expect(hasRename).toBe(true);
    });

    it('should enter rename mode when clicking Rename in context menu', async () => {
      // Click the Rename menu item
      await browser.execute(() => {
        const items = document.querySelectorAll('.context-menu-item');
        const renameItem = Array.from(items).find(
          (item) => item.textContent?.trim() === 'Rename'
        ) as HTMLElement;
        if (renameItem) renameItem.click();
      });

      await waitForRenameInput();

      const inputExists = await elementExists('.tab.active input.tab-title.editing');
      expect(inputExists).toBe(true);

      // Clean up: cancel the rename
      await cancelRename();
      await browser.pause(300);
    });
  });

  describe('Rename with multiple tabs', () => {
    it('should only rename the active tab', async () => {
      // Create a second tab if we don't have one
      const tabCount = await getElementCount('.tab');
      if (tabCount < 2) {
        await createNewTerminalTab();
      }

      // Switch to the first tab
      await browser.execute(() => {
        const firstTab = document.querySelector('.tab') as HTMLElement;
        if (firstTab) firstTab.click();
      });
      await browser.pause(500);

      const otherTabTitle = await browser.execute(() => {
        const tabs = document.querySelectorAll('.tab');
        const secondTab = tabs[1];
        const title = secondTab?.querySelector('.tab-title');
        return title?.textContent?.trim() ?? '';
      });

      // Rename the first tab
      await startRenameByDoubleClick();
      await waitForRenameInput();

      const uniqueName = 'UniqueFirst-' + Date.now();
      await confirmRename(uniqueName);
      await browser.pause(500);

      // Verify the second tab's title is unchanged
      const otherTabTitleAfter = await browser.execute(() => {
        const tabs = document.querySelectorAll('.tab');
        const secondTab = tabs[1];
        const title = secondTab?.querySelector('.tab-title');
        return title?.textContent?.trim() ?? '';
      });

      expect(otherTabTitleAfter).toBe(otherTabTitle);
    });
  });
});
