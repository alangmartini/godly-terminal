import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';

// Mock @tauri-apps/api modules (required for TerminalService import)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { terminalService } from './terminal-service';

// Bug #312: No circuit breaker — stream failures retry forever without
// recovery strategy, visibility-aware probing, or failure state tracking.

/**
 * Helper: create a ReadableStream from an array of Uint8Array chunks.
 */
function chunkedStream(chunks: Uint8Array[]): ReadableStream<Uint8Array> {
  let i = 0;
  return new ReadableStream({
    pull(controller) {
      if (i < chunks.length) {
        controller.enqueue(chunks[i++]);
      } else {
        controller.close();
      }
    },
  });
}

describe('Stream cascade failure — no circuit breaker (Bug #312)', () => {
  let fetchSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSpy = vi.spyOn(globalThis, 'fetch');
  });

  afterEach(() => {
    terminalService.disconnectOutputStream('s1');
    terminalService.disconnectOutputStream('s2');
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('should retry forever on persistent failures without entering a broken state', async () => {
    // Simulate 15 consecutive failures. A circuit breaker should stop retrying
    // after N failures and mark the stream as broken. Current code never stops.
    fetchSpy.mockRejectedValue(new Error('Failed to fetch'));

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // Drive through 15 consecutive failures:
    // Delays: 1000, 2000, 4000, 8000, 10000, 10000, 10000, 10000, 10000, 10000, 10000, 10000, 10000, 10000, 10000
    let totalDelay = 0;
    for (let i = 0; i < 15; i++) {
      await vi.advanceTimersByTimeAsync(0); // let fetch reject
      const delay = Math.min(1000 * Math.pow(2, i), 10000);
      totalDelay += delay;
      await vi.advanceTimersByTimeAsync(delay);
    }

    // All 15+ attempts were made — no circuit breaker stopped them.
    // (First attempt is immediate, then 15 retries after delays.)
    expect(fetchSpy.mock.calls.length).toBeGreaterThanOrEqual(15);

    // There is no "broken" or "circuit open" state on the service.
    // The stream just keeps retrying silently. Verify no state tracking exists.
    expect((terminalService as Record<string, unknown>)['brokenStreams']).toBeUndefined();
    expect((terminalService as Record<string, unknown>)['circuitBreakers']).toBeUndefined();
    expect((terminalService as Record<string, unknown>)['failureCounts']).toBeUndefined();

    controller.abort();
    await promise;
  });

  it('should not distinguish between transient and persistent failures', async () => {
    // A circuit breaker should treat 10+ consecutive failures differently
    // than a single failure followed by recovery. Current code treats
    // all failures the same — just exponential backoff, no state machine.
    fetchSpy.mockRejectedValue(new Error('Failed to fetch'));

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // Fail 10 times — accumulate delays through the exponential backoff.
    // After 10 failures, the delay is capped at 10000ms.
    for (let i = 0; i < 10; i++) {
      await vi.advanceTimersByTimeAsync(0); // let fetch reject
      const delay = Math.min(1000 * Math.pow(2, i), 10000);
      await vi.advanceTimersByTimeAsync(delay); // wait through backoff
    }

    // After 10 consecutive failures, verify the service has no concept
    // of "persistently broken" — no failure count, no circuit state.
    // It just keeps retrying the same way regardless of failure history.
    expect((terminalService as Record<string, unknown>)['failureCounts']).toBeUndefined();
    expect((terminalService as Record<string, unknown>)['circuitState']).toBeUndefined();

    // The service also has no "recovery probing" phase after reconnection.
    // After a success following many failures, it immediately goes back to
    // full-speed streaming with no cautious ramp-up.
    const serviceSource = readFileSync(
      resolve(__dirname, 'terminal-service.ts'),
      'utf-8',
    );
    expect(serviceSource).not.toMatch(/recoveryPhase|halfOpen|probing|slowStart/i);

    controller.abort();
    await promise;
  });

  it('should not trigger immediate reconnection when tab becomes visible', async () => {
    // When the user switches to a tab with a broken stream, the stream
    // should immediately attempt to reconnect (visibility-aware probing).
    // Current code has no visibility awareness — it just waits for the
    // next backoff timer to fire, which could be up to 10 seconds.

    // Verify the source code has no document.visibilitychange listener.
    const serviceSource = readFileSync(
      resolve(__dirname, 'terminal-service.ts'),
      'utf-8',
    );
    expect(serviceSource).not.toContain('visibilitychange');
    expect(serviceSource).not.toContain('visibilityState');

    // Also verify no IntersectionObserver for per-terminal visibility.
    expect(serviceSource).not.toContain('IntersectionObserver');
  });

  it('should have no mechanism to report stream health to the UI', async () => {
    // When all streams are broken, the user sees blank terminals with no
    // indication of what went wrong. There should be a health status
    // (e.g., "stream disconnected", "reconnecting in 8s") exposed to the UI.
    const serviceSource = readFileSync(
      resolve(__dirname, 'terminal-service.ts'),
      'utf-8',
    );

    // No health/status reporting mechanism exists.
    expect(serviceSource).not.toMatch(/streamHealth|streamStatus|connectionState/i);
    expect(serviceSource).not.toMatch(/onStreamError|onStreamDisconnect/i);
  });

  it('should keep growing backoff but never change strategy (current broken behavior)', async () => {
    fetchSpy.mockRejectedValue(new Error('Failed to fetch'));

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // After 5 failures, delay is capped at 10s. From here on, it just
    // repeats 10s delays forever. No escalation (e.g., switch to polling,
    // request full snapshot, or give up and show error to user).
    const delays = [1000, 2000, 4000, 8000, 10000, 10000, 10000];
    for (const delay of delays) {
      await vi.advanceTimersByTimeAsync(0);
      await vi.advanceTimersByTimeAsync(delay);
    }

    // 8 total fetch calls (1 initial + 7 retries)
    expect(fetchSpy.mock.calls.length).toBeGreaterThanOrEqual(8);

    // Verify the service never transitions to a different recovery strategy.
    // It's the same fetch-retry loop from attempt 1 to attempt 1000.
    const serviceSource = readFileSync(
      resolve(__dirname, 'terminal-service.ts'),
      'utf-8',
    );
    expect(serviceSource).not.toMatch(/circuitBreaker|circuitOpen|halfOpen/i);
    expect(serviceSource).not.toMatch(/fallbackToPolling|switchToSnapshot/i);

    controller.abort();
    await promise;
  });

  it('should not coordinate reconnection across multiple sessions', async () => {
    // When the daemon restarts, ALL streams fail simultaneously. Each
    // stream retries independently, causing a thundering herd of
    // reconnection attempts. A coordinator should stagger retries.
    const serviceSource = readFileSync(
      resolve(__dirname, 'terminal-service.ts'),
      'utf-8',
    );

    // No coordination mechanism for reconnection across sessions.
    expect(serviceSource).not.toMatch(/reconnectCoordinator|jitter|stagger/i);
    expect(serviceSource).not.toMatch(/globalBackoff|sharedRetry/i);
  });
});
