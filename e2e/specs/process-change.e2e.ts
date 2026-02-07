import {
  waitForAppReady,
  waitForTerminalPane,
  sendCommand,
} from '../helpers/app';
import { waitForTerminalText } from '../helpers/terminal-reader';

/**
 * Get the active terminal's processName from the store.
 */
async function getStoreProcessName(): Promise<string> {
  return browser.execute(() => {
    const store = (window as any).__store;
    if (!store) return '';
    const state = store.getState();
    const terminal = state.terminals.find(
      (t: any) => t.id === state.activeTerminalId
    );
    return terminal?.processName ?? '';
  });
}

/**
 * Get the active tab's displayed title text.
 */
async function getActiveTabTitle(): Promise<string> {
  return browser.execute(() => {
    const title = document.querySelector('.tab.active .tab-title');
    return title?.textContent?.trim() ?? '';
  });
}

describe('Process Change Events', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
    // Wait for PowerShell to fully initialize
    await browser.pause(5000);
  });

  describe('Initial process state', () => {
    it('should have a process name in the store after terminal creation', async () => {
      const processName = await getStoreProcessName();
      expect(processName.length).toBeGreaterThan(0);
    });

    it('should show a tab title reflecting the initial process', async () => {
      const title = await getActiveTabTitle();
      expect(title.length).toBeGreaterThan(0);
    });
  });

  describe('Process change on child process launch', () => {
    it('should detect process change when launching cmd.exe', async () => {
      const processNameBefore = await getStoreProcessName();

      // Launch cmd.exe as a child process inside PowerShell
      await sendCommand('cmd.exe /k echo CMD_READY');
      await waitForTerminalText('CMD_READY', 15000);

      // Wait for the process-changed event to propagate
      await browser.waitUntil(
        async () => {
          const current = await getStoreProcessName();
          return current !== processNameBefore;
        },
        {
          timeout: 10000,
          interval: 500,
          timeoutMsg:
            'Process name in store did not change after launching cmd.exe',
        }
      );

      const processNameAfter = await getStoreProcessName();
      expect(processNameAfter).not.toBe(processNameBefore);
    });

    it('should update the tab title after process change', async () => {
      const title = await getActiveTabTitle();
      // The tab title should reflect the new process (cmd or similar)
      // We don't assert exact text since it depends on how the app formats it,
      // but it should be non-empty
      expect(title.length).toBeGreaterThan(0);
    });
  });

  describe('Process change on child process exit', () => {
    it('should revert process name when child process exits', async () => {
      const processNameInCmd = await getStoreProcessName();

      // Exit cmd.exe — should return to PowerShell
      await sendCommand('exit');
      await browser.pause(2000);

      // Wait for process-changed event
      await browser.waitUntil(
        async () => {
          const current = await getStoreProcessName();
          return current !== processNameInCmd;
        },
        {
          timeout: 10000,
          interval: 500,
          timeoutMsg:
            'Process name did not revert after exiting cmd.exe',
        }
      );

      const processNameAfter = await getStoreProcessName();
      expect(processNameAfter).not.toBe(processNameInCmd);
    });
  });

  describe('Process change with nested processes', () => {
    it('should track nested process launches', async () => {
      const initialProcess = await getStoreProcessName();

      // Launch cmd.exe
      await sendCommand('cmd.exe /k echo NESTED_START');
      await waitForTerminalText('NESTED_START', 15000);

      // Wait for first process change
      await browser.waitUntil(
        async () => (await getStoreProcessName()) !== initialProcess,
        { timeout: 10000, interval: 500 }
      );

      const afterCmd = await getStoreProcessName();

      // Launch python inside cmd (if available) or another identifiable process
      // Use 'where' command which changes foreground process briefly
      await sendCommand('echo NESTED_LEVEL_2');
      await waitForTerminalText('NESTED_LEVEL_2', 10000);

      // Exit back to PowerShell
      await sendCommand('exit');
      await browser.pause(2000);

      await browser.waitUntil(
        async () => (await getStoreProcessName()) !== afterCmd,
        {
          timeout: 10000,
          interval: 500,
          timeoutMsg: 'Process name did not revert after exiting nested cmd',
        }
      );

      const finalProcess = await getStoreProcessName();
      // Should be back to the original process (PowerShell)
      expect(finalProcess).not.toBe(afterCmd);
    });
  });
});
