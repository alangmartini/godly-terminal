// @vitest-environment jsdom
/**
 * Reproduction tests for bug #531: FPS counter in PerfOverlay always shows 0
 * when idle, and shows artificially low values when typing.
 *
 * Root cause: perfTracer.tick() was only called from TerminalRenderer.render(),
 * which only fires when the daemon pushes new output. The FPS counter had no
 * independent animation frame loop, so it read 0 whenever no data flowed.
 *
 * Fix: PerfOverlay now runs its own rAF loop that calls perfTracer.tick() on
 * every animation frame, independent of data-driven renders.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { perfTracer } from '../utils/PerfTracer';
import { PerfOverlay } from './PerfOverlay';

// Fake timers including rAF and performance.now() for deterministic FPS
const FAKE_TIMER_OPTS = {
  toFake: [
    'setTimeout',
    'clearTimeout',
    'setInterval',
    'clearInterval',
    'requestAnimationFrame',
    'cancelAnimationFrame',
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
   * With the fix, the PerfOverlay runs its own rAF loop that calls tick()
   * on every animation frame (~60/sec), so FPS should show ~60 even when idle.
   */
  it('should show non-zero FPS even when no terminal output is flowing', () => {
    overlay.mount(container);

    // Advance 1 second — rAF callbacks fire at ~16ms intervals (~60 ticks),
    // then the 1000ms setInterval triggers refresh() which reads the count.
    vi.advanceTimersByTime(1000);

    const fps = readFps();
    // With a continuous rAF loop, FPS should reflect display refresh rate
    expect(fps).toBeGreaterThan(0);
  });

  /**
   * Bug #531 (part 2): FPS should reflect animation frame rate (~60), not
   * the number of data-driven renders. The rAF loop ticks independently
   * of terminal output.
   */
  it('should report FPS based on animation frames, not data-driven render count', () => {
    overlay.mount(container);

    // Advance 1 second — rAF loop produces ~60 ticks regardless of output
    vi.advanceTimersByTime(1000);

    const fps = readFps();
    // Should be close to 60 (rAF fires at ~16ms intervals)
    expect(fps).toBeGreaterThan(30);
  });

  /**
   * Bug #531 (part 3): After a burst of typing followed by idle, FPS should
   * NOT drop to 0. The rAF loop keeps ticking regardless of data flow.
   */
  it('should not drop to 0 FPS after terminal output stops', () => {
    overlay.mount(container);

    // First second: rAF loop ticking
    vi.advanceTimersByTime(1000);

    const fpsFirstSecond = readFps();
    expect(fpsFirstSecond).toBeGreaterThan(0);

    // Second second: still ticking (rAF loop is independent of data)
    vi.advanceTimersByTime(1000);

    const fpsSecondSecond = readFps();
    // Should still be non-zero — rAF loop doesn't stop when data stops
    expect(fpsSecondSecond).toBeGreaterThan(0);
  });

  /**
   * The rAF loop must stop when the overlay is destroyed to avoid leaking
   * animation frame callbacks.
   */
  it('should stop the rAF loop when destroyed', () => {
    overlay.mount(container);
    vi.advanceTimersByTime(500);

    const countBefore = perfTracer.getFrameCount();
    expect(countBefore).toBeGreaterThan(0);

    overlay.destroy();

    // Advance more time — no more ticks should accumulate
    const countAfterDestroy = perfTracer.getFrameCount();
    vi.advanceTimersByTime(500);
    const countLater = perfTracer.getFrameCount();
    expect(countLater).toBe(countAfterDestroy);
  });
});
