/**
 * Performance tracer for the terminal render pipeline.
 *
 * Always-on profiling with negligible overhead (~1-2μs per mark/measure).
 * Collects rolling-window samples per span for P50/P95/P99 percentiles.
 *
 * Uses performance.mark() / performance.measure() for DevTools integration.
 * Data is consumed by the PerfOverlay HUD (Ctrl+Shift+P).
 */

const WINDOW_SIZE = 200; // samples per span for percentile calculation

export interface SpanStats {
  avg: number;
  min: number;
  max: number;
  p50: number;
  p95: number;
  p99: number;
  count: number;
}

class RollingWindow {
  private samples: Float64Array;
  private head = 0;
  private size = 0;

  constructor(capacity: number) {
    this.samples = new Float64Array(capacity);
  }

  push(value: number): void {
    this.samples[this.head] = value;
    this.head = (this.head + 1) % this.samples.length;
    if (this.size < this.samples.length) this.size++;
  }

  getStats(): SpanStats | null {
    if (this.size === 0) return null;

    // Copy active samples and sort for percentile calculation
    const active = new Float64Array(this.size);
    if (this.size < this.samples.length) {
      active.set(this.samples.subarray(0, this.size));
    } else {
      // Buffer is full — copy in order from oldest to newest
      const tail = this.samples.length - this.head;
      active.set(this.samples.subarray(this.head, this.head + tail), 0);
      active.set(this.samples.subarray(0, this.head), tail);
    }

    active.sort();

    let total = 0;
    for (let i = 0; i < active.length; i++) total += active[i];

    return {
      avg: total / active.length,
      min: active[0],
      max: active[active.length - 1],
      p50: active[Math.floor(active.length * 0.5)],
      p95: active[Math.floor(active.length * 0.95)],
      p99: active[Math.min(active.length - 1, Math.floor(active.length * 0.99))],
      count: this.size,
    };
  }

  clear(): void {
    this.head = 0;
    this.size = 0;
  }
}

class PerfTracerImpl {
  private windows: Map<string, RollingWindow> = new Map();
  private marks: Map<string, number> = new Map();
  private frameCount = 0;

  /** Place a named mark at the current instant. */
  mark(name: string): void {
    const now = performance.now();
    this.marks.set(name, now);
    try {
      performance.mark(`perf:${name}`);
    } catch {
      // performance.mark can throw if name collides; ignore
    }
  }

  /**
   * Measure elapsed time between a previous mark and now.
   * Returns the duration in milliseconds, or -1 if the start mark is missing.
   */
  measure(spanName: string, startMark: string): number {
    const start = this.marks.get(startMark);
    if (start === undefined) return -1;
    const dur = performance.now() - start;

    try {
      performance.measure(`perf:${spanName}`, `perf:${startMark}`);
    } catch {
      // ignore
    }

    this.getOrCreateWindow(spanName).push(dur);
    return dur;
  }

  /** Record a pre-computed duration for a span name. */
  record(spanName: string, durationMs: number): void {
    this.getOrCreateWindow(spanName).push(durationMs);
  }

  /** Call once per rendered frame to count frames. */
  tick(): void {
    this.frameCount++;
  }

  /** Get the current frame count (for FPS calculation). */
  getFrameCount(): number {
    return this.frameCount;
  }

  /** Get rolling-window stats for all spans without clearing. */
  getStats(): Map<string, SpanStats> {
    const result = new Map<string, SpanStats>();
    for (const [name, window] of this.windows) {
      const stats = window.getStats();
      if (stats) result.set(name, stats);
    }
    return result;
  }

  /** Get stats and reset all windows + frame counter. */
  getAndReset(): Map<string, SpanStats> {
    const result = this.getStats();
    for (const window of this.windows.values()) {
      window.clear();
    }
    this.frameCount = 0;
    return result;
  }

  /** Export Chrome Trace Event Format JSON for all recorded performance measures. */
  exportChromeTrace(): string {
    const entries = performance.getEntriesByType('measure');
    const events = entries
      .filter((e) => e.name.startsWith('perf:'))
      .map((e) => ({
        name: e.name.replace('perf:', ''),
        cat: 'perf',
        ph: 'X',
        ts: Math.round(e.startTime * 1000), // μs
        dur: Math.round(e.duration * 1000), // μs
        pid: 1,
        tid: 1,
      }));
    return JSON.stringify({ traceEvents: events });
  }

  private getOrCreateWindow(name: string): RollingWindow {
    let w = this.windows.get(name);
    if (!w) {
      w = new RollingWindow(WINDOW_SIZE);
      this.windows.set(name, w);
    }
    return w;
  }
}

export type PerfTracer = PerfTracerImpl;

/** Singleton perf tracer — always on with negligible overhead. */
export const perfTracer: PerfTracer = new PerfTracerImpl();
