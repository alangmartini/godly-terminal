import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock @tauri-apps/api modules (required for TerminalService import)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import {
  terminalService,
  STREAM_RECONNECT_BASE_MS,
  CIRCUIT_BREAKER_THRESHOLD,
  CIRCUIT_BREAKER_PROBE_INTERVAL_MS,
  _setJitterRng,
  _resetJitterRng,
} from './terminal-service';

/**
 * Helper: create a ReadableStream from an array of Uint8Array chunks.
 * Each chunk is delivered on successive read() calls, then the stream closes.
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

/**
 * Helper: create a ReadableStream that never closes (hangs on read).
 * Useful for testing abort/disconnect behavior.
 */
function hangingStream(): ReadableStream<Uint8Array> {
  return new ReadableStream({
    pull() {
      // Never resolves — simulates a long-lived stream.
      return new Promise(() => {});
    },
  });
}

describe('TerminalService stream consumer', () => {
  let fetchSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSpy = vi.spyOn(globalThis, 'fetch');
    // Zero jitter for deterministic delay assertions in existing tests.
    _setJitterRng(() => 0);
  });

  afterEach(async () => {
    // Disconnect all streams to prevent dangling promises.
    terminalService.disconnectOutputStream('s1');
    terminalService.disconnectOutputStream('s2');
    // Let abort handlers fire and clean up circuit breaker state.
    await vi.advanceTimersByTimeAsync(0);
    _resetJitterRng();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('should call onData for each non-empty chunk from the stream', async () => {
    const chunks = [
      new Uint8Array([65, 66, 67]),
      new Uint8Array([68, 69]),
    ];
    fetchSpy.mockResolvedValueOnce(
      new Response(chunkedStream(chunks), { status: 200 }),
    );

    const onData = vi.fn();
    const controller = new AbortController();

    // Run _consumeStream directly to await its completion.
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // Let microtasks resolve (stream reads are async).
    await vi.advanceTimersByTimeAsync(0);

    // Stream delivered 2 chunks then closed. The loop will try to reconnect
    // after the stream ends — abort to stop the loop.
    controller.abort();
    await promise;

    expect(onData).toHaveBeenCalledTimes(2);
  });

  it('should not call onData for empty chunks', async () => {
    const chunks = [
      new Uint8Array([]),
      new Uint8Array([1]),
      new Uint8Array([]),
    ];
    fetchSpy.mockResolvedValueOnce(
      new Response(chunkedStream(chunks), { status: 200 }),
    );

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    await vi.advanceTimersByTimeAsync(0);
    controller.abort();
    await promise;

    // Only the non-empty chunk should trigger onData.
    expect(onData).toHaveBeenCalledTimes(1);
  });

  it('should reconnect with exponential backoff on fetch failure', async () => {
    // All three attempts fail — we verify backoff timing.
    fetchSpy.mockRejectedValue(new Error('network error'));

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // First failure → wait 1000ms.
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1000);
    // Second failure → wait 2000ms.
    expect(fetchSpy).toHaveBeenCalledTimes(2);

    await vi.advanceTimersByTimeAsync(2000);
    // Third failure → wait 4000ms.
    expect(fetchSpy).toHaveBeenCalledTimes(3);

    controller.abort();
    await promise;

    expect(onData).not.toHaveBeenCalled();
  });

  it('should stop reconnecting when signal is aborted', async () => {
    // Fetch always fails.
    fetchSpy.mockRejectedValue(new Error('network error'));

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // Let the first attempt fail.
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    // Abort during the reconnect wait.
    controller.abort();
    await promise;

    // No more fetch attempts after abort.
    expect(fetchSpy).toHaveBeenCalledTimes(1);
    expect(onData).not.toHaveBeenCalled();
  });

  it('should reconnect after stream closes cleanly', async () => {
    // First stream delivers one chunk then closes. Second stream hangs.
    fetchSpy
      .mockResolvedValueOnce(
        new Response(chunkedStream([new Uint8Array([1])]), { status: 200 }),
      )
      .mockResolvedValueOnce(
        new Response(hangingStream(), { status: 200 }),
      );

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // First stream delivers chunk and closes.
    await vi.advanceTimersByTimeAsync(0);
    expect(onData).toHaveBeenCalledTimes(1);
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    // Reconnect delay after clean close (reset to base = 1000ms).
    await vi.advanceTimersByTimeAsync(1000);
    expect(fetchSpy).toHaveBeenCalledTimes(2);

    controller.abort();
    await promise;
  });

  it('connectOutputStream replaces existing connection for same session', async () => {
    fetchSpy.mockResolvedValue(
      new Response(hangingStream(), { status: 200 }),
    );

    const onData1 = vi.fn();
    const onData2 = vi.fn();

    terminalService.connectOutputStream('s1', onData1);
    await vi.advanceTimersByTimeAsync(0);

    // Second connect should abort the first.
    terminalService.connectOutputStream('s1', onData2);
    await vi.advanceTimersByTimeAsync(0);

    // The fetch for the second connection should have been called.
    // Total: 2 fetches (one per connect call).
    expect(fetchSpy).toHaveBeenCalledTimes(2);
  });

  it('disconnectOutputStream aborts a live stream', async () => {
    fetchSpy.mockResolvedValue(
      new Response(hangingStream(), { status: 200 }),
    );

    const onData = vi.fn();
    terminalService.connectOutputStream('s1', onData);
    await vi.advanceTimersByTimeAsync(0);

    terminalService.disconnectOutputStream('s1');

    // fetch was called once; no reconnect should happen.
    const callsBefore = fetchSpy.mock.calls.length;
    await vi.advanceTimersByTimeAsync(5000);
    expect(fetchSpy).toHaveBeenCalledTimes(callsBefore);
  });

  it('disconnectOutputStream is safe to call with no active stream', () => {
    expect(() => terminalService.disconnectOutputStream('nonexistent')).not.toThrow();
  });

  it('should use the correct stream URL with session ID', async () => {
    fetchSpy.mockResolvedValueOnce(
      new Response(chunkedStream([]), { status: 200 }),
    );

    const controller = new AbortController();
    const promise = terminalService._consumeStream('my-session-123', controller.signal, vi.fn());

    await vi.advanceTimersByTimeAsync(0);
    controller.abort();
    await promise;

    expect(fetchSpy).toHaveBeenCalledWith(
      'stream://localhost/terminal-output/my-session-123',
      expect.objectContaining({ signal: expect.any(AbortSignal) }),
    );
  });

  it('should handle HTTP error responses and reconnect', async () => {
    fetchSpy
      .mockResolvedValueOnce(new Response(null, { status: 500 }))
      .mockResolvedValueOnce(
        new Response(chunkedStream([new Uint8Array([1])]), { status: 200 }),
      );

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // First attempt → 500 → reconnect wait.
    await vi.advanceTimersByTimeAsync(1000);

    // Second attempt succeeds.
    await vi.advanceTimersByTimeAsync(0);

    controller.abort();
    await promise;

    expect(onData).toHaveBeenCalledTimes(1);
  });

  it('should cap reconnect delay at 10 seconds', async () => {
    // All attempts fail.
    fetchSpy.mockRejectedValue(new Error('fail'));

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // Delays: 1000, 2000, 4000, 8000 (backoff), then circuit breaker opens
    // at failure 5 and switches to probe interval (10000ms).
    await vi.advanceTimersByTimeAsync(0);    // attempt 1
    await vi.advanceTimersByTimeAsync(1000); // attempt 2
    await vi.advanceTimersByTimeAsync(2000); // attempt 3
    await vi.advanceTimersByTimeAsync(4000); // attempt 4
    await vi.advanceTimersByTimeAsync(8000); // attempt 5 (circuit breaker opens)
    expect(fetchSpy).toHaveBeenCalledTimes(5);

    // After circuit breaker opens, delay is probe interval (10000ms).
    await vi.advanceTimersByTimeAsync(10000); // attempt 6 (probe)
    expect(fetchSpy).toHaveBeenCalledTimes(6);

    controller.abort();
    await promise;
  });
});

describe('TerminalService circuit breaker', () => {
  let fetchSpy: ReturnType<typeof vi.spyOn>;

  beforeEach(() => {
    vi.useFakeTimers();
    fetchSpy = vi.spyOn(globalThis, 'fetch');
    // Zero jitter for deterministic delay assertions in circuit breaker tests.
    _setJitterRng(() => 0);
  });

  afterEach(async () => {
    terminalService.disconnectOutputStream('s1');
    terminalService.disconnectOutputStream('s2');
    await vi.advanceTimersByTimeAsync(0);
    _resetJitterRng();
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  /**
   * Helper: advance timers to trigger exactly N consecutive fetch failures.
   * Assumes fetch always rejects. Returns the expected delay for the NEXT
   * wait after the Nth failure. Uses the exact same backoff schedule as
   * _consumeStream: 1000, 2000, 4000, 8000, then probe interval once open.
   *
   * The _consumeStream loop works as:
   *   fetch() → catch → wait(delay) → delay*=2 → loop
   * So the wait after failure K uses the delay that was current BEFORE doubling.
   */
  async function advanceThroughFailures(n: number): Promise<void> {
    // Backoff delays: [1000, 2000, 4000, 8000, ...]
    // After failure i, the wait uses the current delay, then delay doubles.
    // Circuit breaker opens at failure THRESHOLD, switching to probe interval.
    const delays: number[] = [];
    let d = STREAM_RECONNECT_BASE_MS;
    for (let i = 0; i < n; i++) {
      if (i >= CIRCUIT_BREAKER_THRESHOLD) {
        delays.push(CIRCUIT_BREAKER_PROBE_INTERVAL_MS);
      } else {
        delays.push(d);
        d = Math.min(d * 2, 10_000);
      }
    }

    // Attempt 1: fires on first microtask flush.
    await vi.advanceTimersByTimeAsync(0);

    // Attempts 2..n: each fires after the previous failure's wait delay.
    for (let i = 1; i < n; i++) {
      await vi.advanceTimersByTimeAsync(delays[i - 1]);
    }
  }

  it('should open circuit breaker after CIRCUIT_BREAKER_THRESHOLD consecutive failures', async () => {
    fetchSpy.mockRejectedValue(new Error('fail'));

    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, vi.fn());

    // Advance through exactly THRESHOLD failures.
    await advanceThroughFailures(CIRCUIT_BREAKER_THRESHOLD);

    const cb = terminalService.getCircuitBreakerState('s1');
    expect(cb).toBeDefined();
    expect(cb!.open).toBe(true);
    expect(cb!.failures).toBe(CIRCUIT_BREAKER_THRESHOLD);

    controller.abort();
    await promise;
  });

  it('should not open circuit breaker before reaching threshold', async () => {
    fetchSpy.mockRejectedValue(new Error('fail'));

    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, vi.fn());

    // Advance through THRESHOLD - 1 failures.
    await advanceThroughFailures(CIRCUIT_BREAKER_THRESHOLD - 1);

    const cb = terminalService.getCircuitBreakerState('s1');
    expect(cb).toBeDefined();
    expect(cb!.open).toBe(false);
    expect(cb!.failures).toBe(CIRCUIT_BREAKER_THRESHOLD - 1);

    controller.abort();
    await promise;
  });

  it('should use probe interval (not exponential backoff) in open state', async () => {
    fetchSpy.mockRejectedValue(new Error('fail'));

    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, vi.fn());

    // Open the circuit breaker.
    await advanceThroughFailures(CIRCUIT_BREAKER_THRESHOLD);
    expect(fetchSpy).toHaveBeenCalledTimes(CIRCUIT_BREAKER_THRESHOLD);

    // In open state, the next attempt should happen after CIRCUIT_BREAKER_PROBE_INTERVAL_MS.
    // Advancing by less should NOT trigger a new attempt.
    await vi.advanceTimersByTimeAsync(CIRCUIT_BREAKER_PROBE_INTERVAL_MS - 1);
    expect(fetchSpy).toHaveBeenCalledTimes(CIRCUIT_BREAKER_THRESHOLD);

    // Advancing the remaining 1ms triggers the probe.
    await vi.advanceTimersByTimeAsync(1);
    expect(fetchSpy).toHaveBeenCalledTimes(CIRCUIT_BREAKER_THRESHOLD + 1);

    // Next probe also at CIRCUIT_BREAKER_PROBE_INTERVAL_MS (no exponential growth).
    await vi.advanceTimersByTimeAsync(CIRCUIT_BREAKER_PROBE_INTERVAL_MS);
    expect(fetchSpy).toHaveBeenCalledTimes(CIRCUIT_BREAKER_THRESHOLD + 2);

    controller.abort();
    await promise;
  });

  it('should close circuit breaker and reset failures on successful connection', async () => {
    // First THRESHOLD attempts fail, then one succeeds.
    for (let i = 0; i < CIRCUIT_BREAKER_THRESHOLD; i++) {
      fetchSpy.mockRejectedValueOnce(new Error('fail'));
    }
    fetchSpy.mockResolvedValueOnce(
      new Response(hangingStream(), { status: 200 }),
    );

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // Advance through THRESHOLD failures to open the breaker.
    await advanceThroughFailures(CIRCUIT_BREAKER_THRESHOLD);
    expect(terminalService.getCircuitBreakerState('s1')!.open).toBe(true);

    // Advance through probe interval to trigger the successful attempt.
    await vi.advanceTimersByTimeAsync(CIRCUIT_BREAKER_PROBE_INTERVAL_MS);

    // The circuit breaker should now be closed.
    const cb = terminalService.getCircuitBreakerState('s1');
    expect(cb).toBeDefined();
    expect(cb!.open).toBe(false);
    expect(cb!.failures).toBe(0);

    controller.abort();
    await promise;
  });

  it('should reset backoff delay to base after successful reconnection', async () => {
    // Fail twice, succeed (stream closes), then fail once more.
    // After the success, the backoff delay should reset to STREAM_RECONNECT_BASE_MS,
    // so the next retry after the post-success failure uses 1000ms (base), not 4000ms.
    fetchSpy
      .mockRejectedValueOnce(new Error('fail'))   // attempt 1: fail
      .mockRejectedValueOnce(new Error('fail'))   // attempt 2: fail
      .mockResolvedValueOnce(                      // attempt 3: succeed then close
        new Response(chunkedStream([new Uint8Array([1])]), { status: 200 }),
      )
      .mockRejectedValueOnce(new Error('fail'))   // attempt 4: fail (after reconnect)
      .mockResolvedValueOnce(                      // attempt 5: succeed (hangs)
        new Response(hangingStream(), { status: 200 }),
      );

    const onData = vi.fn();
    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, onData);

    // Attempt 1 fails immediately.
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    // Wait 1000ms (base) for attempt 2.
    await vi.advanceTimersByTimeAsync(1000);
    expect(fetchSpy).toHaveBeenCalledTimes(2);

    // Wait 2000ms (doubled) for attempt 3 (success).
    await vi.advanceTimersByTimeAsync(2000);
    expect(fetchSpy).toHaveBeenCalledTimes(3);
    // Let the stream read microtasks complete.
    await vi.advanceTimersByTimeAsync(0);
    expect(onData).toHaveBeenCalledTimes(1);

    // Stream closed cleanly. Delay was reset to STREAM_RECONNECT_BASE_MS on success.
    // Wait base delay (1000ms) for attempt 4 (fail).
    await vi.advanceTimersByTimeAsync(STREAM_RECONNECT_BASE_MS);
    expect(fetchSpy).toHaveBeenCalledTimes(4);

    // After attempt 4 fails with delay=base, the delay doubles to 2000ms.
    // Wait base delay — should NOT trigger attempt 5 yet (delay is now 2000ms).
    await vi.advanceTimersByTimeAsync(STREAM_RECONNECT_BASE_MS);
    expect(fetchSpy).toHaveBeenCalledTimes(4);

    // Wait the remaining 1000ms to hit 2000ms total — triggers attempt 5.
    await vi.advanceTimersByTimeAsync(STREAM_RECONNECT_BASE_MS);
    expect(fetchSpy).toHaveBeenCalledTimes(5);

    controller.abort();
    await promise;
  });

  it('should trigger immediate probe when triggerProbe is called in open state', async () => {
    fetchSpy.mockRejectedValue(new Error('fail'));

    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, vi.fn());

    // Open the circuit breaker.
    await advanceThroughFailures(CIRCUIT_BREAKER_THRESHOLD);
    const callsAfterOpen = fetchSpy.mock.calls.length;
    expect(terminalService.getCircuitBreakerState('s1')!.open).toBe(true);

    // Wait a bit (less than probe interval), then trigger probe.
    await vi.advanceTimersByTimeAsync(500);
    expect(fetchSpy.mock.calls.length).toBe(callsAfterOpen);

    // triggerProbe should wake up the sleep immediately.
    terminalService.triggerProbe('s1');
    await vi.advanceTimersByTimeAsync(0);
    expect(fetchSpy.mock.calls.length).toBe(callsAfterOpen + 1);

    controller.abort();
    await promise;
  });

  it('triggerProbe should be no-op when circuit breaker is closed', async () => {
    fetchSpy.mockRejectedValue(new Error('fail'));

    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, vi.fn());

    // Advance to trigger the first failure and let the backoff timer start.
    await vi.advanceTimersByTimeAsync(0);
    const callsAfterFirstFailure = fetchSpy.mock.calls.length;

    // Circuit breaker should be closed (only 1 failure).
    const cb = terminalService.getCircuitBreakerState('s1');
    expect(cb).toBeDefined();
    expect(cb!.open).toBe(false);

    // triggerProbe should do nothing when circuit breaker is closed.
    terminalService.triggerProbe('s1');
    await vi.advanceTimersByTimeAsync(0);

    // No additional fetch calls should have been made.
    expect(fetchSpy.mock.calls.length).toBe(callsAfterFirstFailure);

    controller.abort();
    await promise;
  });

  it('triggerProbe should be no-op for unknown session', () => {
    // Should not throw.
    expect(() => terminalService.triggerProbe('nonexistent')).not.toThrow();
  });

  it('should clean up circuit breaker state after disconnect', async () => {
    fetchSpy.mockRejectedValue(new Error('fail'));

    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, vi.fn());

    // Open the circuit breaker.
    await advanceThroughFailures(CIRCUIT_BREAKER_THRESHOLD);
    expect(terminalService.getCircuitBreakerState('s1')!.open).toBe(true);

    // Abort (disconnect).
    controller.abort();
    await promise;

    // Circuit breaker state should be cleaned up.
    expect(terminalService.getCircuitBreakerState('s1')).toBeUndefined();
  });

  it('should log when circuit breaker opens (console.warn)', async () => {
    const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
    fetchSpy.mockRejectedValue(new Error('fail'));

    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, vi.fn());

    await advanceThroughFailures(CIRCUIT_BREAKER_THRESHOLD);

    expect(warnSpy).toHaveBeenCalledWith(
      expect.stringContaining('Circuit breaker OPEN for s1'),
    );

    controller.abort();
    await promise;
    warnSpy.mockRestore();
  });

  it('should log when circuit breaker closes (console.info)', async () => {
    const infoSpy = vi.spyOn(console, 'info').mockImplementation(() => {});

    // THRESHOLD failures then success.
    for (let i = 0; i < CIRCUIT_BREAKER_THRESHOLD; i++) {
      fetchSpy.mockRejectedValueOnce(new Error('fail'));
    }
    fetchSpy.mockResolvedValueOnce(
      new Response(hangingStream(), { status: 200 }),
    );

    const controller = new AbortController();
    const promise = terminalService._consumeStream('s1', controller.signal, vi.fn());

    // Open the breaker.
    await advanceThroughFailures(CIRCUIT_BREAKER_THRESHOLD);

    // Probe succeeds.
    await vi.advanceTimersByTimeAsync(CIRCUIT_BREAKER_PROBE_INTERVAL_MS);

    expect(infoSpy).toHaveBeenCalledWith(
      expect.stringContaining('Circuit breaker CLOSED for s1'),
    );

    controller.abort();
    await promise;
    infoSpy.mockRestore();
  });
});
