import { describe, it, expect, beforeEach } from 'vitest';

// Import the class directly since the singleton is always-on
// We'll test through the singleton export
import { perfTracer } from './PerfTracer';

describe('PerfTracer', () => {
  beforeEach(() => {
    // Reset by getting and discarding stats
    perfTracer.getAndReset();
  });

  describe('mark and measure', () => {
    it('should record a span duration between mark and measure', () => {
      perfTracer.mark('test_start');
      // Small delay to ensure non-zero duration
      const result = perfTracer.measure('test_span', 'test_start');
      expect(result).toBeGreaterThanOrEqual(0);

      const stats = perfTracer.getStats();
      expect(stats.has('test_span')).toBe(true);
      const span = stats.get('test_span')!;
      expect(span.count).toBe(1);
      expect(span.avg).toBeGreaterThanOrEqual(0);
    });

    it('should return -1 when start mark is missing', () => {
      const result = perfTracer.measure('missing_span', 'nonexistent_mark');
      expect(result).toBe(-1);
    });

    it('should accumulate multiple measurements', () => {
      for (let i = 0; i < 5; i++) {
        perfTracer.mark('multi_start');
        perfTracer.measure('multi_span', 'multi_start');
      }

      const stats = perfTracer.getStats();
      const span = stats.get('multi_span')!;
      expect(span.count).toBe(5);
    });
  });

  describe('record', () => {
    it('should record pre-computed durations', () => {
      perfTracer.record('manual_span', 10);
      perfTracer.record('manual_span', 20);
      perfTracer.record('manual_span', 30);

      const stats = perfTracer.getStats();
      const span = stats.get('manual_span')!;
      expect(span.count).toBe(3);
      expect(span.avg).toBeCloseTo(20, 0);
      expect(span.min).toBe(10);
      expect(span.max).toBe(30);
    });
  });

  describe('percentile calculation', () => {
    it('should compute correct P50/P95/P99 for known data', () => {
      // Record 100 samples: values 1 through 100
      for (let i = 1; i <= 100; i++) {
        perfTracer.record('percentile_test', i);
      }

      const stats = perfTracer.getStats();
      const span = stats.get('percentile_test')!;
      expect(span.count).toBe(100);
      expect(span.min).toBe(1);
      expect(span.max).toBe(100);
      // Math.floor(100 * 0.5) = index 50 → value 51 (1-indexed data)
      expect(span.p50).toBe(51);
      expect(span.p95).toBe(96);
      expect(span.p99).toBe(100);
    });

    it('should handle single sample correctly', () => {
      perfTracer.record('single', 42);

      const stats = perfTracer.getStats();
      const span = stats.get('single')!;
      expect(span.p50).toBe(42);
      expect(span.p95).toBe(42);
      expect(span.p99).toBe(42);
      expect(span.avg).toBe(42);
      expect(span.min).toBe(42);
      expect(span.max).toBe(42);
    });
  });

  describe('rolling window', () => {
    it('should evict old samples when window is full', () => {
      // Fill beyond the 200-sample window with value 1, then add value 1000
      for (let i = 0; i < 200; i++) {
        perfTracer.record('rolling_test', 1);
      }
      // Now push 10 large values — oldest values should be evicted
      for (let i = 0; i < 10; i++) {
        perfTracer.record('rolling_test', 1000);
      }

      const stats = perfTracer.getStats();
      const span = stats.get('rolling_test')!;
      // Window is 200 samples — the 10 large values replaced 10 small ones
      expect(span.count).toBe(200);
      // Min should still be 1 (190 samples of 1 remain)
      expect(span.min).toBe(1);
      // Max should be 1000
      expect(span.max).toBe(1000);
    });
  });

  describe('getAndReset', () => {
    it('should return stats and clear all windows', () => {
      perfTracer.record('reset_test', 5);

      const stats = perfTracer.getAndReset();
      expect(stats.has('reset_test')).toBe(true);
      expect(stats.get('reset_test')!.count).toBe(1);

      // After reset, stats should be empty
      const afterReset = perfTracer.getStats();
      // Windows still exist but are empty
      const span = afterReset.get('reset_test');
      expect(span).toBeUndefined();
    });
  });

  describe('tick and frame count', () => {
    it('should increment frame count on tick', () => {
      const before = perfTracer.getFrameCount();
      perfTracer.tick();
      perfTracer.tick();
      perfTracer.tick();
      expect(perfTracer.getFrameCount()).toBe(before + 3);
    });

    it('should reset frame count on getAndReset', () => {
      perfTracer.tick();
      perfTracer.tick();
      perfTracer.getAndReset();
      expect(perfTracer.getFrameCount()).toBe(0);
    });
  });

  describe('exportChromeTrace', () => {
    it('should return valid JSON with traceEvents array', () => {
      perfTracer.mark('export_start');
      perfTracer.measure('export_span', 'export_start');

      const json = perfTracer.exportChromeTrace();
      const parsed = JSON.parse(json);
      expect(parsed).toHaveProperty('traceEvents');
      expect(Array.isArray(parsed.traceEvents)).toBe(true);

      // Should contain our measure (with perf: prefix stripped)
      const ourEvent = parsed.traceEvents.find(
        (e: { name: string }) => e.name === 'export_span'
      );
      expect(ourEvent).toBeDefined();
      expect(ourEvent.ph).toBe('X');
      expect(ourEvent.cat).toBe('perf');
      expect(typeof ourEvent.ts).toBe('number');
      expect(typeof ourEvent.dur).toBe('number');
    });
  });

  describe('multiple independent spans', () => {
    it('should track independent spans separately', () => {
      perfTracer.record('span_a', 10);
      perfTracer.record('span_b', 100);
      perfTracer.record('span_a', 20);
      perfTracer.record('span_b', 200);

      const stats = perfTracer.getStats();
      expect(stats.get('span_a')!.avg).toBeCloseTo(15);
      expect(stats.get('span_b')!.avg).toBeCloseTo(150);
    });
  });
});
