// @vitest-environment jsdom
/**
 * Reproduction tests for bug #531: FPS counter in PerfOverlay always shows 0
 * when idle, and shows artificially low values when typing.
 *
 * Root cause: perfTracer.tick() is only called from TerminalRenderer.render(),
 * which only fires when the daemon pushes new output. The FPS counter has no
 * independent animation frame loop, so it reads 0 whenever no data flows.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { perfTracer } from '../utils/PerfTracer';
import { PerfOverlay } from './PerfOverlay';

// Fake timers including performance.now() for deterministic FPS calculation
const FAKE_TIMER_OPTS = {
  toFake: [
    'setTimeout',
    'clearTimeout',
    'setInterval',
    'clearInterval',
    'performance',
  ] as const,
};

describe('PerfOverlay FPS counter (bug #531)', () => {
  let overlay: PerfOverlay;
  let container: HTMLElement;

  beforeEach(() => {
    vi.useFakeTimers(FAKE_TIMER_OPTS);
    perfTracer.getAndReset();
    overlay = new PerfOverlay();
    container = document.createElement('div');
    document.body.appendChild(container);
  });

  afterEach(() => {
    overlay.destroy();
    container.remove();
    vi.useRealTimers();
  });

  /** Helper: read FPS value from the overlay DOM. */
  function readFps(): number {
    const fpsLine = container.querySelector('.perf-overlay-fps');
    if (!fpsLine) return -1;
    const match = (fpsLine.textContent ?? '').match(/FPS:\s*(\d+)/);
    return match ? parseInt(match[1], 10) : -1;
  }

  /**
   * Bug #531: FPS shows 0 when idle because perfTracer.tick() is never called
   * unless the daemon pushes output triggering TerminalRenderer.render().
   *
   * Expected: FPS should be non-zero even when no terminal output is flowing,
   * because the display is still refreshing at the monitor's refresh rate.
   */
  it('should show non-zero FPS even when no terminal output is flowing', () => {
    overlay.mount(container);

    // Advance 1 second — triggers the PerfOverlay refresh interval
    // No calls to perfTracer.tick() (terminal is idle)
    vi.advanceTimersByTime(1000);

    const fps = readFps();
    // Bug: FPS = 0 because tick() was never called
    // Expected: FPS > 0 (display is still refreshing)
    expect(fps).toBeGreaterThan(0);
  });

  /**
   * Bug #531 (part 2): Even during typing, FPS is artificially low because
   * tick() counts data-driven renders (gated by IPC latency), not animation
   * frames. With a 60Hz display, FPS should show ~60, not ~10.
   */
  it('should report FPS based on animation frames, not data-driven render count', () => {
    overlay.mount(container);

    // Simulate typing: 10 renders over 1 second (realistic for keystroke echo).
    // Each keystroke triggers a daemon round-trip → perfTracer.tick().
    for (let i = 0; i < 10; i++) {
      perfTracer.tick();
    }

    vi.advanceTimersByTime(1000);

    const fps = readFps();
    // Bug: FPS = 10 (counts only data-driven renders)
    // Expected: FPS ~60 (should count actual animation frames, not data events)
    expect(fps).toBeGreaterThan(30);
  });

  /**
   * Bug #531 (part 3): After a burst of typing followed by idle, FPS drops
   * immediately to 0. A real FPS counter should show ~60 as long as the
   * display is refreshing, regardless of data flow.
   */
  it('should not drop to 0 FPS after terminal output stops', () => {
    overlay.mount(container);

    // First second: some terminal output (30 renders)
    for (let i = 0; i < 30; i++) {
      perfTracer.tick();
    }
    vi.advanceTimersByTime(1000);

    // Verify first interval had non-zero FPS
    const fpsAfterOutput = readFps();
    expect(fpsAfterOutput).toBeGreaterThan(0);

    // Second second: terminal goes idle (no tick calls)
    vi.advanceTimersByTime(1000);

    const fpsAfterIdle = readFps();
    // Bug: FPS drops to 0 because no tick() calls in the second interval
    // Expected: FPS > 0 (display is still refreshing)
    expect(fpsAfterIdle).toBeGreaterThan(0);
  });
});
