/**
 * E2E Input Latency Test
 *
 * Measures real user-perceived input latency by typing characters in the
 * actual Godly Terminal app and timing how long until they appear in the
 * daemon's godly-vt grid (readable via get_grid_text IPC).
 *
 * All timing runs inside browser.executeAsync() using performance.now()
 * to eliminate WebDriver round-trip overhead from measurements.
 *
 * Three scenarios:
 * 1. Idle keystroke echo — baseline typing feel at a quiet prompt
 * 2. Ctrl+C interrupt responsiveness — how fast the terminal recovers
 *    from heavy output when the user presses Ctrl+C
 * 3. Post-flood keystroke echo — typing feel immediately after stopping
 *    heavy output (checks for lingering contention)
 */
import {
  waitForAppReady,
  waitForTerminalPane,
  sendCommand,
} from '../helpers/app';
import { waitForTerminalText } from '../helpers/terminal-reader';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface LatencyMeasurement {
  marker: string;
  latencyMs: number;
}

interface LatencyStats {
  samples: number;
  min: number;
  max: number;
  mean: number;
  median: number;
  p95: number;
  stddev: number;
}

// ---------------------------------------------------------------------------
// Helpers (run in Node.js context)
// ---------------------------------------------------------------------------

function computeStats(latencies: number[]): LatencyStats {
  const sorted = [...latencies].sort((a, b) => a - b);
  const sum = sorted.reduce((a, b) => a + b, 0);
  const mean = sum / sorted.length;
  const variance =
    sorted.reduce((acc, v) => acc + (v - mean) ** 2, 0) / sorted.length;
  const p95Index = Math.ceil(sorted.length * 0.95) - 1;
  const medianIndex = Math.floor(sorted.length / 2);

  return {
    samples: sorted.length,
    min: sorted[0],
    max: sorted[sorted.length - 1],
    mean: Math.round(mean),
    median: sorted[medianIndex],
    p95: sorted[p95Index],
    stddev: Math.round(Math.sqrt(variance)),
  };
}

function printStats(label: string, stats: LatencyStats): void {
  console.log(`\n  --- ${label} ---`);
  console.log(`  Samples : ${stats.samples}`);
  console.log(`  Min     : ${stats.min} ms`);
  console.log(`  Median  : ${stats.median} ms`);
  console.log(`  Mean    : ${stats.mean} ms`);
  console.log(`  P95     : ${stats.p95} ms`);
  console.log(`  Max     : ${stats.max} ms`);
  console.log(`  Stddev  : ${stats.stddev} ms`);
}

/**
 * Measure keystroke echo latency entirely inside the browser context.
 *
 * Sends `marker` to the active terminal via write_to_terminal, then polls
 * get_grid_text every `pollMs` until the marker appears. Returns elapsed
 * time in milliseconds, or a negative error code.
 *
 * Error codes: -1 = no terminal, -2 = no Tauri, -3 = timeout
 */
async function measureKeystrokeLatency(
  marker: string,
  pollMs: number = 10,
  timeoutMs: number = 10000,
): Promise<number> {
  return browser.executeAsync(
    async (
      _marker: string,
      _pollMs: number,
      _timeoutMs: number,
      done: (result: number) => void,
    ) => {
      try {
        const pane = document.querySelector('.terminal-pane.active');
        const terminalId = pane?.getAttribute('data-terminal-id');
        if (!terminalId) {
          done(-1);
          return;
        }
        const invoke = (window as any).__TAURI__?.core?.invoke;
        if (!invoke) {
          done(-2);
          return;
        }

        const start = performance.now();

        // Send the marker to the PTY — shell echoes it at the prompt
        await invoke('write_to_terminal', {
          terminalId,
          data: _marker,
        });

        // Tight-poll the grid until the marker appears
        const deadline = start + _timeoutMs;
        while (performance.now() < deadline) {
          const text: string = await invoke('get_grid_text', {
            terminalId,
            startRow: 0,
            startCol: 0,
            endRow: 999,
            endCol: 999,
          });
          if (text && text.includes(_marker)) {
            done(Math.round(performance.now() - start));
            return;
          }
          await new Promise((r) => setTimeout(r, _pollMs));
        }
        done(-3); // timeout
      } catch {
        done(-99);
      }
    },
    marker,
    pollMs,
    timeoutMs,
  );
}

/**
 * Cancel the current prompt line (Ctrl+C) and wait for a fresh prompt.
 */
async function clearPromptLine(): Promise<void> {
  await browser.executeAsync(
    async (done: (r: string) => void) => {
      try {
        const pane = document.querySelector('.terminal-pane.active');
        const terminalId = pane?.getAttribute('data-terminal-id');
        const invoke = (window as any).__TAURI__?.core?.invoke;
        if (terminalId && invoke) {
          await invoke('write_to_terminal', {
            terminalId,
            data: '\x03',
          });
        }
        done('ok');
      } catch {
        done('err');
      }
    },
  );
  // Wait for shell to process Ctrl+C and show a fresh prompt
  await browser.pause(800);
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('Input Latency', () => {
  before(async () => {
    await waitForAppReady();
    await waitForTerminalPane();
    // PowerShell takes a few seconds to initialize
    await browser.pause(5000);
    await waitForTerminalText('PS ', 30000);
  });

  it('should measure keystroke echo latency at idle prompt', async () => {
    const ITERATIONS = 20;
    const results: LatencyMeasurement[] = [];
    const failures: Array<{ marker: string; code: number }> = [];

    for (let i = 0; i < ITERATIONS; i++) {
      // Short unique marker that won't appear naturally in the prompt
      const marker = `Z${Date.now().toString(36)}${i}Q`;
      const latency = await measureKeystrokeLatency(marker);

      if (latency > 0) {
        results.push({ marker, latencyMs: latency });
      } else {
        failures.push({ marker, code: latency });
      }

      // Clear the typed marker from the prompt before next iteration
      await clearPromptLine();
    }

    // Report
    expect(results.length).toBeGreaterThanOrEqual(
      Math.floor(ITERATIONS * 0.8),
    );

    const stats = computeStats(results.map((r) => r.latencyMs));
    printStats('IDLE KEYSTROKE ECHO (debug build)', stats);

    if (failures.length > 0) {
      console.log(`  Failures: ${failures.length} (codes: ${failures.map((f) => f.code).join(', ')})`);
    }

    // Generous thresholds for debug builds — the value is in the numbers
    expect(stats.median).toBeLessThan(2000);
    expect(stats.p95).toBeLessThan(5000);
  });

  it('should measure Ctrl+C interrupt responsiveness during output flood', async () => {
    const ITERATIONS = 5;
    const results: LatencyMeasurement[] = [];

    for (let i = 0; i < ITERATIONS; i++) {
      // Start a PowerShell command that produces heavy output
      await sendCommand(
        '1..999999 | ForEach-Object { Write-Host "FLOOD_$_" }',
      );
      // Let the flood run for a few seconds
      await browser.pause(3000);

      // Measure: from Ctrl+C to prompt return
      const marker = `PS `;
      const latency = await browser.executeAsync(
        async (
          _pollMs: number,
          _timeoutMs: number,
          done: (result: number) => void,
        ) => {
          try {
            const pane = document.querySelector('.terminal-pane.active');
            const terminalId = pane?.getAttribute('data-terminal-id');
            if (!terminalId) {
              done(-1);
              return;
            }
            const invoke = (window as any).__TAURI__?.core?.invoke;
            if (!invoke) {
              done(-2);
              return;
            }

            const start = performance.now();

            // Send Ctrl+C to interrupt the flood
            await invoke('write_to_terminal', {
              terminalId,
              data: '\x03',
            });

            // Poll until a fresh prompt appears (last line contains "PS ")
            const deadline = start + _timeoutMs;
            while (performance.now() < deadline) {
              const text: string = await invoke('get_grid_text', {
                terminalId,
                startRow: 0,
                startCol: 0,
                endRow: 999,
                endCol: 999,
              });

              // Check if the grid's last non-empty line looks like a prompt
              if (text) {
                const lines = text.split('\n').filter((l: string) => l.trim().length > 0);
                const lastLine = lines[lines.length - 1] || '';
                if (lastLine.includes('PS ') && lastLine.includes('>')) {
                  done(Math.round(performance.now() - start));
                  return;
                }
              }
              await new Promise((r) => setTimeout(r, _pollMs));
            }
            done(-3); // timeout
          } catch {
            done(-99);
          }
        },
        10, // pollMs
        15000, // timeoutMs — flood can take a moment to stop
      );

      if (latency > 0) {
        results.push({ marker: `ctrl_c_${i}`, latencyMs: latency });
      }

      // Wait for prompt to be fully stable before next iteration
      await browser.pause(1000);
    }

    expect(results.length).toBeGreaterThanOrEqual(3);

    const stats = computeStats(results.map((r) => r.latencyMs));
    printStats('CTRL+C INTERRUPT RESPONSIVENESS (debug build)', stats);

    // Generous: in debug builds, stopping a flood can take a moment
    expect(stats.median).toBeLessThan(5000);
  });

  it('should measure keystroke echo latency right after stopping a flood', async () => {
    // First, create a flood and stop it
    await sendCommand(
      '1..999999 | ForEach-Object { Write-Host "FLOOD_$_" }',
    );
    await browser.pause(3000);

    // Stop the flood
    await browser.executeAsync(
      async (done: (r: string) => void) => {
        try {
          const pane = document.querySelector('.terminal-pane.active');
          const terminalId = pane?.getAttribute('data-terminal-id');
          const invoke = (window as any).__TAURI__?.core?.invoke;
          if (terminalId && invoke) {
            await invoke('write_to_terminal', {
              terminalId,
              data: '\x03',
            });
          }
          done('ok');
        } catch {
          done('err');
        }
      },
    );

    // Wait for prompt to return
    await waitForTerminalText('PS ', 15000);
    // Minimal pause — we want to measure immediately after flood stops
    await browser.pause(200);

    // Now measure keystroke echo — same as idle test but right after a flood
    const ITERATIONS = 10;
    const results: LatencyMeasurement[] = [];

    for (let i = 0; i < ITERATIONS; i++) {
      const marker = `P${Date.now().toString(36)}${i}Q`;
      const latency = await measureKeystrokeLatency(marker);

      if (latency > 0) {
        results.push({ marker, latencyMs: latency });
      }

      await clearPromptLine();
    }

    expect(results.length).toBeGreaterThanOrEqual(
      Math.floor(ITERATIONS * 0.7),
    );

    const stats = computeStats(results.map((r) => r.latencyMs));
    printStats('POST-FLOOD KEYSTROKE ECHO (debug build)', stats);

    // Should recover to near-idle performance
    expect(stats.median).toBeLessThan(3000);
  });
});
