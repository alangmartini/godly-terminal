import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// Bug C1: Each terminal-output event triggered a separate xterm.js write() call.
// Under heavy output (hundreds of events/sec), each write() runs the parser
// synchronously, saturating the main thread. Fix: buffer chunks and flush once
// per animation frame.
//
// This test exercises the buffering + flush logic extracted from TerminalPane.

/** Minimal reproduction of the RAF-batched output buffer used in TerminalPane */
function createOutputBuffer(write: (data: Uint8Array) => void) {
  let buffer: Uint8Array[] = [];
  let rafId: number | null = null;

  function flush() {
    rafId = null;
    const chunks = buffer;
    if (chunks.length === 0) return;
    buffer = [];

    if (chunks.length === 1) {
      write(chunks[0]);
      return;
    }

    let totalLength = 0;
    for (const chunk of chunks) {
      totalLength += chunk.byteLength;
    }
    const merged = new Uint8Array(totalLength);
    let offset = 0;
    for (const chunk of chunks) {
      merged.set(chunk, offset);
      offset += chunk.byteLength;
    }
    write(merged);
  }

  return {
    push(data: Uint8Array) {
      buffer.push(data);
      if (rafId === null) {
        rafId = requestAnimationFrame(() => flush());
      }
    },
    cancel() {
      if (rafId !== null) {
        cancelAnimationFrame(rafId);
        rafId = null;
      }
      buffer = [];
    },
    /** Expose for assertions */
    get pendingChunks() { return buffer.length; },
    get scheduled() { return rafId !== null; },
  };
}

describe('Output buffer (RAF-batched writes)', () => {
  let rafCallbacks: Array<() => void>;
  let nextRafId: number;

  beforeEach(() => {
    rafCallbacks = [];
    nextRafId = 1;

    vi.stubGlobal('requestAnimationFrame', (cb: () => void) => {
      const id = nextRafId++;
      rafCallbacks.push(cb);
      return id;
    });
    vi.stubGlobal('cancelAnimationFrame', (id: number) => {
      // For simplicity, clear all pending (tests use at most one RAF at a time)
      rafCallbacks = [];
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function flushRAF() {
    const cbs = rafCallbacks.splice(0);
    cbs.forEach(cb => cb());
  }

  it('batches multiple chunks into a single write() per frame', () => {
    const write = vi.fn();
    const buf = createOutputBuffer(write);

    buf.push(new Uint8Array([65]));      // 'A'
    buf.push(new Uint8Array([66, 67]));  // 'BC'
    buf.push(new Uint8Array([68]));      // 'D'

    // No write yet — still buffering
    expect(write).not.toHaveBeenCalled();
    expect(buf.pendingChunks).toBe(3);

    // Simulate animation frame
    flushRAF();

    expect(write).toHaveBeenCalledTimes(1);
    expect(write).toHaveBeenCalledWith(new Uint8Array([65, 66, 67, 68]));
    expect(buf.pendingChunks).toBe(0);
  });

  it('passes through a single chunk without concatenation overhead', () => {
    const write = vi.fn();
    const buf = createOutputBuffer(write);
    const chunk = new Uint8Array([72, 101, 108, 108, 111]); // 'Hello'

    buf.push(chunk);
    flushRAF();

    expect(write).toHaveBeenCalledTimes(1);
    // Should pass the original chunk reference, not a copy
    expect(write).toHaveBeenCalledWith(chunk);
  });

  it('schedules only one RAF even with many pushes', () => {
    const write = vi.fn();
    const buf = createOutputBuffer(write);

    for (let i = 0; i < 100; i++) {
      buf.push(new Uint8Array([i]));
    }

    // Only one RAF callback should have been registered
    expect(rafCallbacks.length).toBe(1);

    flushRAF();
    expect(write).toHaveBeenCalledTimes(1);
    expect(write.mock.calls[0][0].byteLength).toBe(100);
  });

  it('allows new writes after a flush', () => {
    const write = vi.fn();
    const buf = createOutputBuffer(write);

    // First batch
    buf.push(new Uint8Array([1, 2]));
    flushRAF();
    expect(write).toHaveBeenCalledTimes(1);

    // Second batch
    buf.push(new Uint8Array([3, 4]));
    buf.push(new Uint8Array([5]));
    flushRAF();
    expect(write).toHaveBeenCalledTimes(2);
    expect(write.mock.calls[1][0]).toEqual(new Uint8Array([3, 4, 5]));
  });

  it('cancel() discards pending buffer and RAF', () => {
    const write = vi.fn();
    const buf = createOutputBuffer(write);

    buf.push(new Uint8Array([1]));
    buf.push(new Uint8Array([2]));
    expect(buf.scheduled).toBe(true);

    buf.cancel();
    expect(buf.pendingChunks).toBe(0);
    expect(buf.scheduled).toBe(false);

    // Even if RAF fires (shouldn't, but defensive), no write occurs
    flushRAF();
    expect(write).not.toHaveBeenCalled();
  });

  it('handles empty flush gracefully (no-op)', () => {
    const write = vi.fn();
    createOutputBuffer(write);

    // Force a flush with nothing in the buffer
    flushRAF();
    expect(write).not.toHaveBeenCalled();
  });
});
