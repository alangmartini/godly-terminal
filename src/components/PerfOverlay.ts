/**
 * Performance HUD overlay.
 *
 * Displays real-time latency percentiles for key operations.
 * Toggled via Ctrl+Shift+P (debug.togglePerfOverlay shortcut).
 */

import { perfTracer, type SpanStats } from '../utils/PerfTracer';

/** Preferred display order and human-readable labels for span names. */
const SPAN_LABELS: [string, string][] = [
  ['keydown_to_paint', 'keydown\u2192paint'],
  ['write_to_terminal_ipc', 'write ipc'],
  ['keydown_to_output', 'key\u2192output'],
  ['snapshot_ipc', 'snapshot ipc'],
  ['paint_duration', 'paint'],
  ['raf_wait', 'raf wait'],
  ['create_terminal', 'new terminal'],
  ['tab_switch', 'tab switch'],
  ['workspace_switch', 'ws switch'],
  ['app_startup', 'app startup'],
  ['reconnect_sessions', 'reconnect'],
];

const LABEL_MAP = new Map(SPAN_LABELS);
const ORDERED_KEYS = SPAN_LABELS.map(([k]) => k);

function fmt(ms: number): string {
  if (ms >= 1000) return (ms / 1000).toFixed(1) + 's';
  if (ms >= 100) return ms.toFixed(0);
  if (ms >= 10) return ms.toFixed(1);
  return ms.toFixed(1);
}

export class PerfOverlay {
  private el: HTMLElement;
  private tableBody: HTMLElement;
  private fpsLine: HTMLElement;
  private interval: ReturnType<typeof setInterval> | null = null;
  private lastTickTime = performance.now();
  private lastFrameCount = 0;

  constructor() {
    this.el = document.createElement('div');
    this.el.className = 'perf-overlay';

    // Header
    const header = document.createElement('div');
    header.className = 'perf-overlay-header';
    header.textContent = 'Perf';

    // Export button
    const exportBtn = document.createElement('button');
    exportBtn.className = 'perf-overlay-export';
    exportBtn.textContent = 'Export';
    exportBtn.title = 'Export Chrome trace (open in chrome://tracing)';
    exportBtn.addEventListener('click', (e) => {
      e.stopPropagation();
      this.exportTrace();
    });
    header.appendChild(exportBtn);

    this.el.appendChild(header);

    // Table header
    const thead = document.createElement('div');
    thead.className = 'perf-overlay-row perf-overlay-thead';
    thead.innerHTML =
      '<span class="perf-col-name">metric</span>' +
      '<span class="perf-col-num">p50</span>' +
      '<span class="perf-col-num">p95</span>' +
      '<span class="perf-col-num">max</span>' +
      '<span class="perf-col-num">n</span>';
    this.el.appendChild(thead);

    // Table body
    this.tableBody = document.createElement('div');
    this.tableBody.className = 'perf-overlay-body';
    this.el.appendChild(this.tableBody);

    // FPS line
    this.fpsLine = document.createElement('div');
    this.fpsLine.className = 'perf-overlay-fps';
    this.fpsLine.textContent = 'FPS: --';
    this.el.appendChild(this.fpsLine);
  }

  mount(parent: HTMLElement): void {
    parent.appendChild(this.el);
    this.lastTickTime = performance.now();
    this.lastFrameCount = perfTracer.getFrameCount();
    this.interval = setInterval(() => this.refresh(), 1000);
    this.refresh();
  }

  destroy(): void {
    if (this.interval) {
      clearInterval(this.interval);
      this.interval = null;
    }
    this.el.remove();
  }

  private refresh(): void {
    const stats = perfTracer.getStats();

    // Build rows: ordered keys first, then any unknown spans
    const rows: [string, SpanStats][] = [];
    for (const key of ORDERED_KEYS) {
      const s = stats.get(key);
      if (s) rows.push([key, s]);
    }
    for (const [key, s] of stats) {
      if (!LABEL_MAP.has(key)) rows.push([key, s]);
    }

    this.tableBody.textContent = '';
    for (const [name, s] of rows) {
      const row = document.createElement('div');
      row.className = 'perf-overlay-row';
      const label = LABEL_MAP.get(name) ?? name;
      row.innerHTML =
        `<span class="perf-col-name">${label}</span>` +
        `<span class="perf-col-num">${fmt(s.p50)}</span>` +
        `<span class="perf-col-num">${fmt(s.p95)}</span>` +
        `<span class="perf-col-num">${fmt(s.max)}</span>` +
        `<span class="perf-col-num">${s.count}</span>`;
      this.tableBody.appendChild(row);
    }

    // FPS
    const now = performance.now();
    const elapsed = (now - this.lastTickTime) / 1000;
    const frames = perfTracer.getFrameCount() - this.lastFrameCount;
    const fps = elapsed > 0 ? Math.round(frames / elapsed) : 0;
    this.lastTickTime = now;
    this.lastFrameCount = perfTracer.getFrameCount();
    this.fpsLine.textContent = `FPS: ${fps}`;
  }

  private exportTrace(): void {
    const json = perfTracer.exportChromeTrace();
    const blob = new Blob([json], { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `godly-perf-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  }
}
