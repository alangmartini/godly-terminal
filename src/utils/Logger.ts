import { invoke } from '@tauri-apps/api/core';

const FLUSH_INTERVAL_MS = 500;
const FLUSH_THRESHOLD = 50;

let buffer: string[] = [];

function timestamp(): string {
  const now = new Date();
  const y = now.getFullYear();
  const mo = String(now.getMonth() + 1).padStart(2, '0');
  const d = String(now.getDate()).padStart(2, '0');
  const h = String(now.getHours()).padStart(2, '0');
  const mi = String(now.getMinutes()).padStart(2, '0');
  const s = String(now.getSeconds()).padStart(2, '0');
  const ms = String(now.getMilliseconds()).padStart(3, '0');
  return `${y}-${mo}-${d} ${h}:${mi}:${s}.${ms}`;
}

function formatArgs(args: unknown[]): string {
  return args
    .map((a) => {
      if (typeof a === 'string') return a;
      try {
        return JSON.stringify(a);
      } catch {
        return String(a);
      }
    })
    .join(' ');
}

function enqueue(level: string, args: unknown[]): void {
  const line = `[${timestamp()}] [${level}] ${formatArgs(args)}`;
  buffer.push(line);
  if (buffer.length >= FLUSH_THRESHOLD) {
    flush();
  }
}

function flush(): void {
  if (buffer.length === 0) return;
  const lines = buffer.splice(0);
  invoke('write_frontend_log', { lines }).catch(() => {
    // If IPC fails (e.g. during shutdown), silently drop — no retry loops.
  });
}

const originalLog = console.log.bind(console);
const originalWarn = console.warn.bind(console);
const originalError = console.error.bind(console);
const originalDebug = console.debug.bind(console);

export function initLogger(): void {
  console.log = (...args: unknown[]) => {
    originalLog(...args);
    enqueue('INFO', args);
  };

  console.warn = (...args: unknown[]) => {
    originalWarn(...args);
    enqueue('WARN', args);
  };

  console.error = (...args: unknown[]) => {
    originalError(...args);
    enqueue('ERROR', args);
  };

  console.debug = (...args: unknown[]) => {
    originalDebug(...args);
    enqueue('DEBUG', args);
  };

  window.onerror = (_msg, source, lineno, colno, error) => {
    enqueue('ERROR', [
      `Uncaught: ${error?.message ?? _msg} at ${source}:${lineno}:${colno}`,
    ]);
    flush();
  };

  window.onunhandledrejection = (event: PromiseRejectionEvent) => {
    enqueue('ERROR', [`Unhandled rejection: ${event.reason}`]);
    flush();
  };

  window.addEventListener('beforeunload', () => {
    flush();
  });

  setInterval(flush, FLUSH_INTERVAL_MS);

  enqueue('INFO', ['Frontend logger initialized']);
}
