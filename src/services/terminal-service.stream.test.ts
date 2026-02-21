import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Mock @tauri-apps/api modules (required for TerminalService import)
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

import { terminalService } from './terminal-service';

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
  });

  afterEach(() => {
    // Disconnect all streams to prevent dangling promises.
    terminalService.disconnectOutputStream('s1');
    terminalService.disconnectOutputStream('s2');
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

    // Delays: 1000, 2000, 4000, 8000, 10000 (capped), 10000, ...
    await vi.advanceTimersByTimeAsync(0);    // attempt 1
    await vi.advanceTimersByTimeAsync(1000); // attempt 2
    await vi.advanceTimersByTimeAsync(2000); // attempt 3
    await vi.advanceTimersByTimeAsync(4000); // attempt 4
    await vi.advanceTimersByTimeAsync(8000); // attempt 5
    expect(fetchSpy).toHaveBeenCalledTimes(5);

    // Next delay should be capped at 10000ms, not 16000ms.
    await vi.advanceTimersByTimeAsync(10000); // attempt 6
    expect(fetchSpy).toHaveBeenCalledTimes(6);

    controller.abort();
    await promise;
  });
});
