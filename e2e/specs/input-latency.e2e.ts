/**
 * E2E Input Latency Test
 *
 * Measures real user-perceived input latency across the full pipeline:
 *   keydown event → JS handler → Tauri IPC → daemon pipe → PTY echo →
 *   godly-vt parse → terminal-output event → snapshot fetch → RAF → Canvas paint
 *
 * Two measurement approaches, reported side by side:
 *
 * 1. "Key-to-Grid" — dispatches synthetic KeyboardEvent on the terminal canvas,
 *    polls get_grid_text until the character appears. Covers the full input path
 *    (JS handler + IPC + daemon + PTY echo + grid update) but not the render.
 *
 * 2. "Key-to-Pixel" — dispatches synthetic KeyboardEvent on the terminal canvas,
 *    polls canvas pixels at the cursor position until they change. Covers the
 *    ENTIRE pipeline including RAF scheduling and Canvas paint — this is what the
 *    user actually perceives.
 *
 * All timing runs inside browser.executeAsync() using performance.now().
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
// Stats (run in Node.js context)
// ---------------------------------------------------------------------------

function computeStats(latencies: number[]): LatencyStats {
  if (latencies.length === 0) {
    return { samples: 0, min: 0, max: 0, mean: 0, median: 0, p95: 0, stddev: 0 };
  }
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

// ---------------------------------------------------------------------------
// Measurement helpers (run inside browser via executeAsync)
// ---------------------------------------------------------------------------

/**
 * Measure KEY-TO-PIXEL latency: dispatches a synthetic KeyboardEvent on the
 * terminal canvas, then polls canvas pixels at the cursor position until they
 * change. This measures the FULL user-perceived pipeline.
 *
 * Uses an offscreen canvas drawImage() trick to read pixels from both
 * WebGL and Canvas2D main canvases.
 *
 * Returns latency in ms, or negative error code.
 */
async function measureKeyToPixelLatency(
  char: string,
  pollMs: number = 5,
  timeoutMs: number = 10000,
): Promise<number> {
  return browser.executeAsync(
    async (
      _char: string,
      _pollMs: number,
      _timeoutMs: number,
      done: (result: number) => void,
    ) => {
      try {
        const pane = document.querySelector('.terminal-pane.active');
        if (!pane) { done(-1); return; }
        const terminalId = pane.getAttribute('data-terminal-id');
        if (!terminalId) { done(-1); return; }
        const invoke = (window as any).__TAURI__?.core?.invoke;
        if (!invoke) { done(-2); return; }

        // Find the terminal canvas (the focused element that receives keydown)
        const canvas = pane.querySelector('canvas.terminal-canvas') as HTMLCanvasElement;
        if (!canvas) { done(-4); return; }

        // Get cursor position and grid dimensions from daemon
        const snapshot = await invoke('get_grid_snapshot', { terminalId });
        const cursorRow: number = snapshot.cursor.row;
        const cursorCol: number = snapshot.cursor.col;
        const gridRows: number = snapshot.dimensions.rows;
        const gridCols: number = snapshot.dimensions.cols;

        // Calculate pixel coordinates of the cursor cell
        // canvas.width/height are in device pixels
        const cellW = Math.round(canvas.width / gridCols);
        const cellH = Math.round(canvas.height / gridRows);
        const pixelX = cursorCol * cellW;
        const pixelY = cursorRow * cellH;

        // Create an offscreen canvas to read pixels (works for both WebGL and Canvas2D)
        const probe = document.createElement('canvas');
        probe.width = cellW;
        probe.height = cellH;
        const probeCtx = probe.getContext('2d', { willReadFrequently: true });
        if (!probeCtx) { done(-5); return; }

        // Snapshot the cursor cell pixels BEFORE the keystroke
        probeCtx.drawImage(canvas, pixelX, pixelY, cellW, cellH, 0, 0, cellW, cellH);
        const beforeData = probeCtx.getImageData(0, 0, cellW, cellH).data;
        const beforeCopy = new Uint8Array(beforeData);

        // Dispatch synthetic KeyboardEvent on the canvas (same path as real user)
        const start = performance.now();
        canvas.dispatchEvent(new KeyboardEvent('keydown', {
          key: _char,
          code: `Key${_char.toUpperCase()}`,
          bubbles: true,
          cancelable: true,
        }));

        // Tight-poll: read cursor cell pixels until they change
        const deadline = start + _timeoutMs;
        const poll = async () => {
          while (performance.now() < deadline) {
            // Small yield to let RAF and paint happen
            await new Promise(r => setTimeout(r, _pollMs));

            probeCtx.clearRect(0, 0, cellW, cellH);
            probeCtx.drawImage(canvas, pixelX, pixelY, cellW, cellH, 0, 0, cellW, cellH);
            const afterData = probeCtx.getImageData(0, 0, cellW, cellH).data;

            // Compare pixel data
            let changed = false;
            for (let i = 0; i < beforeCopy.length; i++) {
              if (beforeCopy[i] !== afterData[i]) {
                changed = true;
                break;
              }
            }
            if (changed) {
              done(Math.round(performance.now() - start));
              return;
            }
          }
          done(-3); // timeout
        };
        poll();
      } catch {
        done(-99);
      }
    },
    char,
    pollMs,
    timeoutMs,
  );
}

/**
 * Measure KEY-TO-GRID latency: dispatches a synthetic KeyboardEvent, then
 * polls get_grid_text until the character appears. Covers JS handler +
 * IPC + daemon + PTY echo + grid update, but NOT the render pipeline.
 *
 * Returns latency in ms, or negative error code.
 */
async function measureKeyToGridLatency(
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
        if (!pane) { done(-1); return; }
        const terminalId = pane.getAttribute('data-terminal-id');
        if (!terminalId) { done(-1); return; }
        const invoke = (window as any).__TAURI__?.core?.invoke;
        if (!invoke) { done(-2); return; }

        // Find the terminal canvas
        const canvas = pane.querySelector('canvas.terminal-canvas') as HTMLCanvasElement;
        if (!canvas) { done(-4); return; }

        const start = performance.now();

        // Dispatch one keydown per character (same path as real typing)
        for (const ch of _marker) {
          canvas.dispatchEvent(new KeyboardEvent('keydown', {
            key: ch,
            code: ch.match(/[a-z]/i) ? `Key${ch.toUpperCase()}` : '',
            bubbles: true,
            cancelable: true,
          }));
        }

        // Poll the grid until the full marker appears
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
          await new Promise(r => setTimeout(r, _pollMs));
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
          await invoke('write_to_terminal', { terminalId, data: '\x03' });
        }
        done('ok');
      } catch {
        done('err');
      }
    },
  );
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

  it('should measure idle keystroke latency (key-to-pixel)', async () => {
    const ITERATIONS = 20;
    const results: LatencyMeasurement[] = [];
    const failures: Array<{ code: number }> = [];

    // Characters to cycle through — avoids repeating the same one
    const chars = 'abcdefghijklmnopqrst';

    for (let i = 0; i < ITERATIONS; i++) {
      const ch = chars[i % chars.length];
      const latency = await measureKeyToPixelLatency(ch);

      if (latency > 0) {
        results.push({ marker: ch, latencyMs: latency });
      } else {
        failures.push({ code: latency });
      }

      // Clear the typed character from the prompt
      await clearPromptLine();
    }

    expect(results.length).toBeGreaterThanOrEqual(Math.floor(ITERATIONS * 0.7));

    const stats = computeStats(results.map(r => r.latencyMs));

    console.log('\n========================================');
    console.log('  INPUT LATENCY E2E — IDLE PROMPT');
    console.log('========================================');
    printStats('Key-to-Pixel (full user-perceived, debug build)', stats);

    if (failures.length > 0) {
      console.log(`  Failures: ${failures.length} (codes: ${failures.map(f => f.code).join(', ')})`);
    }
    console.log('========================================');

    // Generous thresholds for debug builds — the value is in the reported numbers
    expect(stats.median).toBeLessThan(2000);
    expect(stats.p95).toBeLessThan(5000);
  });

  it('should measure idle keystroke latency (key-to-grid)', async () => {
    const ITERATIONS = 20;
    const results: LatencyMeasurement[] = [];
    const failures: Array<{ code: number }> = [];

    for (let i = 0; i < ITERATIONS; i++) {
      const marker = `Z${Date.now().toString(36)}${i}Q`;
      const latency = await measureKeyToGridLatency(marker);

      if (latency > 0) {
        results.push({ marker, latencyMs: latency });
      } else {
        failures.push({ code: latency });
      }

      await clearPromptLine();
    }

    expect(results.length).toBeGreaterThanOrEqual(Math.floor(ITERATIONS * 0.7));

    const stats = computeStats(results.map(r => r.latencyMs));
    printStats('Key-to-Grid (JS handler + backend pipeline, debug build)', stats);

    if (failures.length > 0) {
      console.log(`  Failures: ${failures.length} (codes: ${failures.map(f => f.code).join(', ')})`);
    }

    expect(stats.median).toBeLessThan(2000);
  });

  it('should measure Ctrl+C interrupt responsiveness during output flood', async () => {
    const ITERATIONS = 5;
    const results: LatencyMeasurement[] = [];

    for (let i = 0; i < ITERATIONS; i++) {
      // Start a PowerShell command that floods output
      await sendCommand(
        '1..999999 | ForEach-Object { Write-Host "FLOOD_$_" }',
      );
      await browser.pause(3000);

      // Measure: from Ctrl+C keydown on canvas to prompt visible in pixels
      const latency = await browser.executeAsync(
        async (
          _pollMs: number,
          _timeoutMs: number,
          done: (result: number) => void,
        ) => {
          try {
            const pane = document.querySelector('.terminal-pane.active');
            if (!pane) { done(-1); return; }
            const terminalId = pane.getAttribute('data-terminal-id');
            if (!terminalId) { done(-1); return; }
            const invoke = (window as any).__TAURI__?.core?.invoke;
            if (!invoke) { done(-2); return; }

            const start = performance.now();

            // Send Ctrl+C via the real keyboard handler path
            const canvas = pane.querySelector('canvas.terminal-canvas') as HTMLCanvasElement;
            if (canvas) {
              canvas.dispatchEvent(new KeyboardEvent('keydown', {
                key: 'c',
                code: 'KeyC',
                ctrlKey: true,
                bubbles: true,
                cancelable: true,
              }));
            } else {
              // Fallback to direct IPC
              await invoke('write_to_terminal', { terminalId, data: '\x03' });
            }

            // Poll until a fresh prompt appears
            const deadline = start + _timeoutMs;
            while (performance.now() < deadline) {
              const text: string = await invoke('get_grid_text', {
                terminalId,
                startRow: 0,
                startCol: 0,
                endRow: 999,
                endCol: 999,
              });
              if (text) {
                const lines = text.split('\n').filter((l: string) => l.trim().length > 0);
                const lastLine = lines[lines.length - 1] || '';
                if (lastLine.includes('PS ') && lastLine.includes('>')) {
                  done(Math.round(performance.now() - start));
                  return;
                }
              }
              await new Promise(r => setTimeout(r, _pollMs));
            }
            done(-3);
          } catch {
            done(-99);
          }
        },
        10,
        15000,
      );

      if (latency > 0) {
        results.push({ marker: `ctrl_c_${i}`, latencyMs: latency });
      }

      await browser.pause(1000);
    }

    expect(results.length).toBeGreaterThanOrEqual(3);

    const stats = computeStats(results.map(r => r.latencyMs));

    console.log('\n========================================');
    console.log('  INPUT LATENCY E2E — DURING FLOOD');
    console.log('========================================');
    printStats('Ctrl+C to Prompt (debug build)', stats);
    console.log('========================================');

    expect(stats.median).toBeLessThan(5000);
  });

  it('should measure post-flood keystroke latency', async () => {
    // Create a flood and stop it
    await sendCommand(
      '1..999999 | ForEach-Object { Write-Host "FLOOD_$_" }',
    );
    await browser.pause(3000);

    // Stop via canvas keydown
    await browser.executeAsync(
      async (done: (r: string) => void) => {
        try {
          const pane = document.querySelector('.terminal-pane.active');
          const canvas = pane?.querySelector('canvas.terminal-canvas') as HTMLCanvasElement;
          if (canvas) {
            canvas.dispatchEvent(new KeyboardEvent('keydown', {
              key: 'c',
              code: 'KeyC',
              ctrlKey: true,
              bubbles: true,
              cancelable: true,
            }));
          }
          done('ok');
        } catch {
          done('err');
        }
      },
    );

    await waitForTerminalText('PS ', 15000);
    await browser.pause(200);

    // Measure key-to-pixel latency immediately after flood
    const ITERATIONS = 10;
    const results: LatencyMeasurement[] = [];
    const chars = 'abcdefghij';

    for (let i = 0; i < ITERATIONS; i++) {
      const ch = chars[i % chars.length];
      const latency = await measureKeyToPixelLatency(ch);

      if (latency > 0) {
        results.push({ marker: ch, latencyMs: latency });
      }

      await clearPromptLine();
    }

    expect(results.length).toBeGreaterThanOrEqual(Math.floor(ITERATIONS * 0.6));

    const stats = computeStats(results.map(r => r.latencyMs));

    console.log('\n========================================');
    console.log('  INPUT LATENCY E2E — POST-FLOOD');
    console.log('========================================');
    printStats('Key-to-Pixel post-flood (debug build)', stats);
    console.log('========================================');

    expect(stats.median).toBeLessThan(3000);
  });
});
