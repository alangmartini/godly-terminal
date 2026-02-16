/**
 * Performance tracer for the terminal render pipeline.
 *
 * Measures wall-clock time at each stage of the keystroke-to-paint path:
 *   keydown -> write_to_terminal IPC -> terminal-output event ->
 *   scheduleSnapshotFetch -> get_grid_snapshot IPC -> CellDataEncoder.encode ->
 *   WebGL paint
 *
 * Enabled via the PERF_TRACE flag. When disabled, every method is a no-op
 * so there is zero overhead in production.
 *
 * Uses performance.mark() / performance.measure() for DevTools integration
 * and logs a summary table every `LOG_INTERVAL` frames.
 */

const PERF_TRACE = (globalThis as Record<string, unknown>).__PERF_TRACE === true;
const LOG_INTERVAL = 100; // frames between summary logs

interface SpanRecord {
  total: number;
  count: number;
  min: number;
  max: number;
}

class PerfTracerImpl {
  private spans: Map<string, SpanRecord> = new Map();
  private frameCount = 0;
  private marks: Map<string, number> = new Map();

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

    let rec = this.spans.get(spanName);
    if (!rec) {
      rec = { total: 0, count: 0, min: Infinity, max: -Infinity };
      this.spans.set(spanName, rec);
    }
    rec.total += dur;
    rec.count += 1;
    if (dur < rec.min) rec.min = dur;
    if (dur > rec.max) rec.max = dur;

    return dur;
  }

  /** Record a pre-computed duration for a span name. */
  record(spanName: string, durationMs: number): void {
    let rec = this.spans.get(spanName);
    if (!rec) {
      rec = { total: 0, count: 0, min: Infinity, max: -Infinity };
      this.spans.set(spanName, rec);
    }
    rec.total += durationMs;
    rec.count += 1;
    if (durationMs < rec.min) rec.min = durationMs;
    if (durationMs > rec.max) rec.max = durationMs;
  }

  /** Call once per rendered frame to trigger periodic logging. */
  tick(): void {
    this.frameCount += 1;
    if (this.frameCount >= LOG_INTERVAL) {
      this.logSummary();
      this.frameCount = 0;
    }
  }

  /** Log summary table and reset accumulators. */
  logSummary(): void {
    if (this.spans.size === 0) return;

    const rows: Record<string, { avg: string; min: string; max: string; count: number }> = {};
    for (const [name, rec] of this.spans) {
      rows[name] = {
        avg: (rec.total / rec.count).toFixed(2) + 'ms',
        min: rec.min.toFixed(2) + 'ms',
        max: rec.max.toFixed(2) + 'ms',
        count: rec.count,
      };
    }
    console.table(rows);
    this.spans.clear();
  }
}

/** No-op implementation with identical API surface. */
const noopTracer = {
  mark(_name: string): void {},
  measure(_spanName: string, _startMark: string): number { return -1; },
  record(_spanName: string, _durationMs: number): void {},
  tick(): void {},
  logSummary(): void {},
};

export type PerfTracer = PerfTracerImpl;

/**
 * Singleton perf tracer. Returns real implementation when
 * `globalThis.__PERF_TRACE = true` is set before module load,
 * otherwise returns a zero-cost no-op object.
 */
export const perfTracer: PerfTracer = PERF_TRACE ? new PerfTracerImpl() : noopTracer as unknown as PerfTracer;
