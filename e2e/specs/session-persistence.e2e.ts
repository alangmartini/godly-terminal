import {
  waitForAppReady,
  waitForTerminalPane,
  sendCommand,
  triggerSave,
} from '../helpers/app';
import {
  getTerminalText,
  waitForTerminalText,
} from '../helpers/terminal-reader';
import { readLayoutFile } from '../helpers/persistence';

const MARKER = 'SESSION_ALIVE_MARKER';

describe('Session Process Persistence', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
  });

  it('should start a long-running loop command', async () => {
    // Wait for PowerShell to fully initialize
    await browser.pause(5000);

    // Start an infinite loop that outputs the marker every 3 seconds
    await sendCommand(
      `while ($true) { Start-Sleep 3; Write-Output "${MARKER}" }`
    );

    // Wait for the first marker to appear — confirms the loop is running
    await waitForTerminalText(MARKER, 30000);

    const text = await getTerminalText();
    expect(text).toContain(MARKER);
  });

  it('should save layout before restart', async () => {
    await triggerSave();

    const layout = readLayoutFile();
    expect(layout).not.toBeNull();

    const data = (layout as any).layout || layout;
    expect(data.terminals).toBeDefined();
    expect(data.terminals.length).toBeGreaterThanOrEqual(1);
  });

  it('should have the loop command still producing output after restart', async () => {
    // Restart the app (close + reopen)
    await browser.reloadSession();
    await waitForAppReady();

    try {
      await waitForTerminalPane();
    } catch {
      // If no terminal pane after restart, the bug is even worse
      const layout = readLayoutFile();
      throw new Error(
        'No terminal pane found after restart. Layout on disk: ' +
          JSON.stringify(layout)
      );
    }

    // Let session reconnection settle
    await browser.pause(5000);

    // Snapshot 1: read current terminal buffer and count markers
    const textBefore = await getTerminalText();
    const countBefore = (textBefore.match(new RegExp(MARKER, 'g')) || [])
      .length;

    // Wait long enough for at least 3 new outputs (3s interval x 3 = ~9s, use 10s)
    await browser.pause(10000);

    // Snapshot 2: read terminal buffer again
    const textAfter = await getTerminalText();
    const countAfter = (textAfter.match(new RegExp(MARKER, 'g')) || []).length;

    // If the loop is still alive, new markers should have appeared
    expect(countAfter).toBeGreaterThan(countBefore);
  });
});
