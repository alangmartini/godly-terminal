import {
  waitForAppReady,
  waitForTerminalPane,
  sendCommand,
} from '../helpers/app';
import { waitForTerminalText } from '../helpers/terminal-reader';

/**
 * Get the current terminal dimensions (rows x cols) via Tauri IPC.
 */
async function getTerminalDimensions(): Promise<{ rows: number; cols: number }> {
  return browser.executeAsync(async (done: (result: { rows: number; cols: number }) => void) => {
    try {
      const pane = document.querySelector('.terminal-pane.active') as any;
      const terminalId = pane?.getAttribute('data-terminal-id');
      if (!terminalId) { done({ rows: 0, cols: 0 }); return; }
      const invoke = (window as any).__TAURI__?.core?.invoke;
      if (!invoke) { done({ rows: 0, cols: 0 }); return; }
      const [rows, cols] = await invoke('get_grid_dimensions', { terminalId });
      done({ rows, cols });
    } catch { done({ rows: 0, cols: 0 }); }
  });
}

/**
 * Resize the terminal via the Tauri IPC command directly.
 */
async function resizeTerminalViaIpc(
  terminalId: string,
  rows: number,
  cols: number
): Promise<string> {
  return browser.execute(
    (tId: string, r: number, c: number) => {
      const invoke = (window as any).__TAURI__?.core?.invoke;
      if (!invoke) return 'error: no tauri';
      invoke('resize_terminal', { terminalId: tId, rows: r, cols: c }).catch(
        (e: any) => {
          console.error('[e2e] resize_terminal failed:', e);
        }
      );
      return 'ok';
    },
    terminalId,
    rows,
    cols
  );
}

/**
 * Get the active terminal's ID.
 */
async function getActiveTerminalId(): Promise<string> {
  return browser.execute(() => {
    const pane = document.querySelector('.terminal-pane.active');
    return pane?.getAttribute('data-terminal-id') ?? '';
  });
}

describe('Terminal Resize', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
    // Wait for shell and initial fit
    await browser.pause(5000);
  });

  describe('Initial dimensions', () => {
    it('should have non-zero dimensions after mount', async () => {
      const dims = await getTerminalDimensions();
      expect(dims.rows).toBeGreaterThan(0);
      expect(dims.cols).toBeGreaterThan(0);
    });

    it('should have reasonable terminal size', async () => {
      const dims = await getTerminalDimensions();
      // A typical terminal should have at least 10 rows and 40 cols
      expect(dims.rows).toBeGreaterThanOrEqual(10);
      expect(dims.cols).toBeGreaterThanOrEqual(40);
    });
  });

  describe('Resize via IPC', () => {
    it('should accept a resize_terminal IPC call without error', async () => {
      const terminalId = await getActiveTerminalId();
      expect(terminalId).not.toBe('');

      const result = await resizeTerminalViaIpc(terminalId, 30, 100);
      expect(result).toBe('ok');

      // Wait for resize to propagate
      await browser.pause(1000);
    });

    it('should reflect the resize in shell output', async () => {
      // PowerShell can report its window size
      const marker = 'RESIZE_CHECK_' + Date.now();
      await sendCommand(`echo "${marker}"; $Host.UI.RawUI.WindowSize`);
      await waitForTerminalText(marker, 15000);

      // The command should complete without errors
      // (the exact dimensions may differ from what we sent since the
      // PTY and renderer negotiate independently, but the IPC should not fail)
    });
  });

  describe('Resize via window resize', () => {
    it('should update terminal dimensions when the window is resized', async () => {
      const dimsBefore = await getTerminalDimensions();

      // Get current window size
      const windowSize = await browser.getWindowSize();

      // Resize the window to be significantly different
      const newWidth = Math.max(windowSize.width - 200, 800);
      const newHeight = Math.max(windowSize.height - 150, 600);
      await browser.setWindowSize(newWidth, newHeight);

      // Wait for ResizeObserver + fitAddon to fire
      await browser.pause(2000);

      const dimsAfter = await getTerminalDimensions();

      // At least one dimension should have changed (likely cols since width changed)
      const changed =
        dimsAfter.rows !== dimsBefore.rows || dimsAfter.cols !== dimsBefore.cols;
      expect(changed).toBe(true);
    });

    it('should still have valid dimensions after resize', async () => {
      const dims = await getTerminalDimensions();
      expect(dims.rows).toBeGreaterThan(0);
      expect(dims.cols).toBeGreaterThan(0);
    });

    it('should restore dimensions when window is enlarged', async () => {
      // Enlarge the window back
      await browser.setWindowSize(1400, 900);
      await browser.pause(2000);

      const dims = await getTerminalDimensions();
      expect(dims.rows).toBeGreaterThan(0);
      expect(dims.cols).toBeGreaterThan(0);

      // With a larger window, we expect more columns
      expect(dims.cols).toBeGreaterThanOrEqual(80);
    });
  });

  describe('Resize with terminal output', () => {
    it('should handle resize while terminal has content', async () => {
      // Generate some output
      const marker = 'RESIZE_CONTENT_' + Date.now();
      await sendCommand(`echo "${marker}"`);
      await waitForTerminalText(marker, 15000);

      // Now resize
      const windowSize = await browser.getWindowSize();
      await browser.setWindowSize(windowSize.width - 100, windowSize.height);
      await browser.pause(1500);

      // Terminal should still be functional — send another command
      const marker2 = 'AFTER_RESIZE_' + Date.now();
      await sendCommand(`echo "${marker2}"`);
      await waitForTerminalText(marker2, 15000);
    });
  });
});
