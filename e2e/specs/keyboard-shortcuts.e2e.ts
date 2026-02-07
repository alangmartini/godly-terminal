import {
  waitForAppReady,
  waitForTerminalPane,
  getElementCount,
  elementExists,
  triggerSave,
} from '../helpers/app';
import { readLayoutFile } from '../helpers/persistence';

describe('Keyboard Shortcuts', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
    // Wait for shell to initialize
    await browser.pause(5000);
  });

  describe('Ctrl+T — New Terminal', () => {
    it('should create a new terminal tab', async () => {
      const countBefore = await getElementCount('.tab');

      await browser.keys(['Control', 't']);
      await browser.waitUntil(
        async () => (await getElementCount('.tab')) > countBefore,
        { timeout: 15000, timeoutMsg: 'Ctrl+T did not create a new tab' }
      );

      const countAfter = await getElementCount('.tab');
      expect(countAfter).toBe(countBefore + 1);
    });

    it('should make the new tab active', async () => {
      const isLastActive = await browser.execute(() => {
        const tabs = document.querySelectorAll('.tab');
        const lastTab = tabs[tabs.length - 1];
        return lastTab?.classList.contains('active') ?? false;
      });
      expect(isLastActive).toBe(true);
    });
  });

  describe('Ctrl+Tab / Ctrl+Shift+Tab — Cycle Terminals', () => {
    before(async () => {
      // Ensure we have at least 2 tabs (created one above already)
      const count = await getElementCount('.tab');
      if (count < 2) {
        await browser.keys(['Control', 't']);
        await browser.waitUntil(
          async () => (await getElementCount('.tab')) >= 2,
          { timeout: 15000 }
        );
      }
    });

    it('should switch to the next terminal with Ctrl+Tab', async () => {
      // Activate the first tab
      await browser.execute(() => {
        const firstTab = document.querySelector('.tab') as HTMLElement;
        if (firstTab) firstTab.click();
      });
      await browser.pause(500);

      const firstId = await browser.execute(() => {
        return document.querySelector('.tab.active')?.getAttribute('data-terminal-id');
      });

      await browser.keys(['Control', 'Tab']);
      await browser.pause(500);

      const nextId = await browser.execute(() => {
        return document.querySelector('.tab.active')?.getAttribute('data-terminal-id');
      });

      expect(nextId).not.toBe(firstId);
    });

    it('should switch to the previous terminal with Ctrl+Shift+Tab', async () => {
      const currentId = await browser.execute(() => {
        return document.querySelector('.tab.active')?.getAttribute('data-terminal-id');
      });

      await browser.keys(['Control', 'Shift', 'Tab']);
      await browser.pause(500);

      const prevId = await browser.execute(() => {
        return document.querySelector('.tab.active')?.getAttribute('data-terminal-id');
      });

      expect(prevId).not.toBe(currentId);
    });

    it('should cycle back to the first terminal after reaching the end', async () => {
      // Activate first tab
      await browser.execute(() => {
        const firstTab = document.querySelector('.tab') as HTMLElement;
        if (firstTab) firstTab.click();
      });
      await browser.pause(500);

      const firstId = await browser.execute(() => {
        return document.querySelector('.tab.active')?.getAttribute('data-terminal-id');
      });

      // Press Ctrl+Tab N times where N = tab count to cycle back
      const tabCount = await getElementCount('.tab');
      for (let i = 0; i < tabCount; i++) {
        await browser.keys(['Control', 'Tab']);
        await browser.pause(300);
      }

      const afterCycleId = await browser.execute(() => {
        return document.querySelector('.tab.active')?.getAttribute('data-terminal-id');
      });

      expect(afterCycleId).toBe(firstId);
    });
  });

  describe('Ctrl+W — Close Terminal', () => {
    it('should close the active terminal tab', async () => {
      // Ensure we have at least 2 tabs so closing one doesn't leave us with zero
      const countBefore = await getElementCount('.tab');
      if (countBefore < 2) {
        await browser.keys(['Control', 't']);
        await browser.waitUntil(
          async () => (await getElementCount('.tab')) >= 2,
          { timeout: 15000 }
        );
      }

      const countBeforeClose = await getElementCount('.tab');
      await browser.keys(['Control', 'w']);

      await browser.waitUntil(
        async () => (await getElementCount('.tab')) < countBeforeClose,
        { timeout: 10000, timeoutMsg: 'Ctrl+W did not close the tab' }
      );

      const countAfter = await getElementCount('.tab');
      expect(countAfter).toBe(countBeforeClose - 1);
    });

    it('should activate another tab after closing', async () => {
      const hasActive = await elementExists('.tab.active');
      expect(hasActive).toBe(true);
    });
  });

  describe('Ctrl+Shift+S — Manual Save', () => {
    it('should save the layout to disk', async () => {
      await browser.keys(['Control', 'Shift', 's']);
      // Wait for the save to complete
      await browser.pause(2000);

      const layout = readLayoutFile();
      expect(layout).not.toBeNull();

      const data = layout.layout || layout;
      expect(data.workspaces).toBeDefined();
      expect(data.workspaces.length).toBeGreaterThanOrEqual(1);
      expect(data.terminals).toBeDefined();
      expect(data.terminals.length).toBeGreaterThanOrEqual(1);
    });
  });

  describe('Ctrl+Shift+L — Manual Load', () => {
    it('should load the layout without error', async () => {
      // First save current state
      await triggerSave();

      // Trigger a load
      await browser.keys(['Control', 'Shift', 'l']);
      await browser.pause(3000);

      // App should still have terminals after load
      const hasTerminals = await elementExists('.tab');
      expect(hasTerminals).toBe(true);

      // No init error should have been thrown
      const error = await browser.execute(() => {
        return (window as any).__app_init_error ?? null;
      });
      expect(error).toBeNull();
    });
  });
});
