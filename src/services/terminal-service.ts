import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { store, ShellType } from '../state/store';
import { perfTracer } from '../utils/PerfTracer';
import type { RichGridData, RichGridDiff } from '../components/TerminalRenderer';
import { decodeAllDiffs } from '../utils/binary-diff-decoder';


// Backend shell type format - matches Rust serde externally tagged enum
export type BackendShellType =
  | 'windows'
  | 'pwsh'
  | 'cmd'
  | { wsl: { distribution: string | null } }
  | { custom: { program: string; args: string[] | null } };

// Convert frontend ShellType to backend format
function toBackendShellType(shellType: ShellType): BackendShellType {
  if (shellType.type === 'windows') return 'windows';
  if (shellType.type === 'pwsh') return 'pwsh';
  if (shellType.type === 'cmd') return 'cmd';
  if (shellType.type === 'custom') return { custom: { program: shellType.program, args: shellType.args ?? null } };
  return { wsl: { distribution: shellType.distribution ?? null } };
}

export interface TerminalOutputPayload {
  terminal_id: string;
}

export interface ProcessChangedPayload {
  terminal_id: string;
  process_name: string;
}

export interface TerminalGridDiffPayload {
  terminal_id: string;
  diff: RichGridDiff;
}

export interface TerminalClosedPayload {
  terminal_id: string;
  exit_code: number | null;
}

/** Result of create_terminal IPC call */
export interface CreateTerminalResult {
  id: string;
  worktree_branch: string | null;
}

/** Info about a live daemon session (from reconnect_sessions) */
export interface SessionInfo {
  id: string;
  shell_type: BackendShellType;
  pid: number;
  rows: number;
  cols: number;
  cwd: string | null;
  created_at: number;
  attached: boolean;
  running: boolean;
}

/** Default delay before first reconnect attempt (ms). */
export const STREAM_RECONNECT_BASE_MS = 1000;
/** Maximum delay between reconnect attempts (ms). */
export const STREAM_RECONNECT_MAX_MS = 10_000;
/** Number of consecutive failures before the circuit breaker opens. */
export const CIRCUIT_BREAKER_THRESHOLD = 5;
/** Probe interval when circuit breaker is open (ms). */
export const CIRCUIT_BREAKER_PROBE_INTERVAL_MS = 10_000;

/** Circuit breaker state for a single stream connection. */
export interface CircuitBreakerState {
  /** Number of consecutive failures since last success. */
  failures: number;
  /** Whether the circuit breaker is in "open" state (polling stopped, probing only). */
  open: boolean;
}

/** Returns a random jitter in [0, range) to break thundering herd patterns. */
let jitterRng = () => Math.random();

/** @internal — override the jitter RNG for deterministic testing. */
export function _setJitterRng(fn: () => number): void {
  jitterRng = fn;
}

/** @internal — restore the default jitter RNG. */
export function _resetJitterRng(): void {
  jitterRng = () => Math.random();
}

class TerminalService {
  private outputListeners: Map<string, () => void> = new Map();
  private gridDiffListeners: Map<string, (diff: RichGridDiff) => void> = new Map();
  private unlistenFns: UnlistenFn[] = [];
  /** AbortControllers for active output stream connections (keyed by session ID). */
  private streamControllers: Map<string, AbortController> = new Map();
  /** AbortControllers for active diff stream connections (keyed by session ID). */
  private diffStreamControllers: Map<string, AbortController> = new Map();
  /** Circuit breaker state per session. @internal — visible for testing. */
  _circuitBreakers: Map<string, CircuitBreakerState> = new Map();
  /**
   * Resolve functions for pending probe wake-ups. When `triggerProbe()` is
   * called for a session in open state, we resolve this to interrupt the
   * probe-interval sleep so the next attempt happens immediately.
   * @internal — visible for testing.
   */
  _probeWakeups: Map<string, () => void> = new Map();

  async init() {
    const unlistenOutput = await listen<TerminalOutputPayload>(
      'terminal-output',
      (event) => {
        const { terminal_id } = event.payload;
        const listener = this.outputListeners.get(terminal_id);
        if (listener) {
          listener();
        }
      }
    );

    const unlistenGridDiff = await listen<TerminalGridDiffPayload>(
      'terminal-grid-diff',
      (event) => {
        const { terminal_id, diff } = event.payload;
        const listener = this.gridDiffListeners.get(terminal_id);
        if (listener) {
          listener(diff);
        }
      }
    );

    const unlistenProcess = await listen<ProcessChangedPayload>(
      'process-changed',
      (event) => {
        const { terminal_id, process_name } = event.payload;
        store.updateTerminal(terminal_id, { processName: process_name });
      }
    );

    const unlistenClosed = await listen<TerminalClosedPayload>(
      'terminal-closed',
      (event) => {
        const { terminal_id, exit_code } = event.payload;
        this.outputListeners.delete(terminal_id);
        store.updateTerminal(terminal_id, {
          exited: true,
          exitCode: exit_code ?? undefined,
        });
        console.info(
          `[TerminalService] Session closed: terminal=${terminal_id}, exit_code=${exit_code}`
        );
        // Free daemon session resources (fire-and-forget)
        invoke('close_terminal', { terminalId: terminal_id }).catch(() => {});
      }
    );

    this.unlistenFns.push(unlistenOutput, unlistenGridDiff, unlistenProcess, unlistenClosed);
  }

  async createTerminal(
    workspaceId: string,
    options?: {
      cwdOverride?: string;
      shellTypeOverride?: ShellType;
      idOverride?: string;
      worktreeName?: string;
      nameOverride?: string;
    }
  ): Promise<CreateTerminalResult> {
    // Only send shellTypeOverride when the caller explicitly provides one.
    // When null, the backend uses the workspace's configured shell type.
    const shellTypeOverride = options?.shellTypeOverride
      ? toBackendShellType(options.shellTypeOverride)
      : null;

    const result = await invoke<CreateTerminalResult>('create_terminal', {
      workspaceId,
      cwdOverride: options?.cwdOverride ?? null,
      shellTypeOverride,
      idOverride: options?.idOverride ?? null,
      worktreeName: options?.worktreeName ?? null,
      nameOverride: options?.nameOverride ?? null,
    });
    return result;
  }

  async closeTerminal(terminalId: string): Promise<void> {
    await invoke('close_terminal', { terminalId });
    this.outputListeners.delete(terminalId);
    // Also delete scrollback data
    try {
      await invoke('delete_scrollback', { terminalId });
    } catch {
      // Ignore scrollback deletion errors
    }
  }

  async writeToTerminal(terminalId: string, data: string): Promise<void> {
    await invoke('write_to_terminal', {
      terminalId,
      data,
    });
  }

  async resizeTerminal(
    terminalId: string,
    rows: number,
    cols: number
  ): Promise<void> {
    await invoke('resize_terminal', {
      terminalId,
      rows,
      cols,
    });
  }

  async renameTerminal(terminalId: string, name: string): Promise<void> {
    await invoke('rename_terminal', { terminalId, name });
    store.updateTerminal(terminalId, { name });
  }

  /** List live daemon sessions (for reconnection on app restart) */
  async reconnectSessions(): Promise<SessionInfo[]> {
    perfTracer.mark('reconnect_start');
    try {
      const result = await invoke<SessionInfo[]>('reconnect_sessions');
      perfTracer.measure('reconnect_sessions', 'reconnect_start');
      return result;
    } catch {
      perfTracer.measure('reconnect_sessions', 'reconnect_start');
      return [];
    }
  }

  /** Attach to an existing daemon session */
  async attachSession(
    sessionId: string,
    workspaceId: string,
    name: string
  ): Promise<void> {
    await invoke('attach_session', { sessionId, workspaceId, name });
  }

  async setScrollback(terminalId: string, offset: number): Promise<void> {
    await invoke('set_scrollback', { terminalId, offset });
  }

  /** Set scrollback offset and return grid snapshot in a single IPC round-trip. */
  async scrollAndGetSnapshot(terminalId: string, offset: number): Promise<RichGridData> {
    return invoke<RichGridData>('scroll_and_get_snapshot', { terminalId, offset });
  }

  /** Pause output streaming for a session (background optimization). */
  async pauseSession(sessionId: string): Promise<void> {
    await invoke('pause_session', { sessionId });
  }

  /** Resume output streaming for a previously paused session. */
  async resumeSession(sessionId: string): Promise<void> {
    await invoke('resume_session', { sessionId });
  }

  onTerminalOutput(terminalId: string, callback: () => void) {
    this.outputListeners.set(terminalId, callback);
    return () => this.outputListeners.delete(terminalId);
  }

  onTerminalGridDiff(terminalId: string, callback: (diff: RichGridDiff) => void) {
    this.gridDiffListeners.set(terminalId, callback);
    return () => this.gridDiffListeners.delete(terminalId);
  }

  /**
   * Connect to the terminal output stream via Tauri custom protocol.
   * Bypasses the Tauri event JSON serialization path — raw bytes arrive
   * as ReadableStream chunks. Each chunk triggers the onData callback,
   * which the caller uses to schedule a grid snapshot fetch.
   *
   * Automatically reconnects with exponential backoff if the stream drops.
   * The existing terminal-output event listener remains as a fallback.
   */
  connectOutputStream(sessionId: string, onData: () => void): void {
    this.disconnectOutputStream(sessionId);

    const controller = new AbortController();
    this.streamControllers.set(sessionId, controller);

    this._consumeStream(sessionId, controller.signal, onData);
  }

  /**
   * Disconnect from the terminal output stream for a session.
   * Safe to call even if no stream is connected.
   */
  disconnectOutputStream(sessionId: string): void {
    const controller = this.streamControllers.get(sessionId);
    if (controller) {
      controller.abort();
      this.streamControllers.delete(sessionId);
    }
    // Circuit breaker state is cleaned up inside _consumeStream when the
    // loop exits (after abort). No need to clear it here — the abort
    // signal causes the loop to break and clean up.
  }

  /**
   * Connect to the binary diff stream via Tauri custom protocol.
   * Binary-encoded RichGridDiff frames arrive as ReadableStream chunks,
   * decoded in-place and delivered to the onDiff callback. This eliminates
   * both JSON serialization and the IPC round-trip for grid snapshots.
   */
  connectDiffStream(sessionId: string, onDiff: (diff: RichGridDiff) => void): void {
    this.disconnectDiffStream(sessionId);

    const controller = new AbortController();
    this.diffStreamControllers.set(sessionId, controller);

    this._consumeDiffStream(sessionId, controller.signal, onDiff);
  }

  /**
   * Disconnect from the binary diff stream for a session.
   */
  disconnectDiffStream(sessionId: string): void {
    const controller = this.diffStreamControllers.get(sessionId);
    if (controller) {
      controller.abort();
      this.diffStreamControllers.delete(sessionId);
    }
  }

  /** @internal */
  async _consumeDiffStream(
    sessionId: string,
    signal: AbortSignal,
    onDiff: (diff: RichGridDiff) => void,
  ): Promise<void> {
    let delay = STREAM_RECONNECT_BASE_MS;

    while (!signal.aborted) {
      try {
        const response = await fetch(
          `stream://localhost/terminal-diff/${sessionId}`,
          { signal },
        );

        if (!response.ok || !response.body) {
          throw new Error(`Diff stream error: ${response.status}`);
        }

        delay = STREAM_RECONNECT_BASE_MS;

        const reader = response.body.getReader();
        const onAbort = () => reader.cancel();
        signal.addEventListener('abort', onAbort, { once: true });
        try {
          while (!signal.aborted) {
            const { done, value } = await reader.read();
            if (done) break;
            if (value && value.length > 0) {
              const diffs = decodeAllDiffs(value);
              for (const diff of diffs) {
                onDiff(diff);
              }
            }
          }
        } finally {
          signal.removeEventListener('abort', onAbort);
          reader.releaseLock();
        }
      } catch (err: unknown) {
        if (signal.aborted) break;

        console.debug(
          `[TerminalService] Diff stream error for ${sessionId}, reconnecting in ~${delay}ms`,
          err instanceof Error ? err.message : err,
        );
      }

      if (signal.aborted) break;

      // Exponential backoff with jitter
      const waitTime = delay + Math.floor(jitterRng() * STREAM_RECONNECT_BASE_MS);
      await new Promise<void>((resolve) => {
        const timer = setTimeout(resolve, waitTime);
        const cleanup = () => { clearTimeout(timer); resolve(); };
        signal.addEventListener('abort', cleanup, { once: true });
      });

      delay = Math.min(delay * 2, STREAM_RECONNECT_MAX_MS);
    }
  }

  /**
   * Get the circuit breaker state for a session, or null if none exists.
   * @internal — visible for testing.
   */
  getCircuitBreakerState(sessionId: string): CircuitBreakerState | undefined {
    return this._circuitBreakers.get(sessionId);
  }

  /**
   * Trigger an immediate probe for a session whose circuit breaker is open.
   * Called when a terminal becomes visible (tab switch) to enable instant
   * recovery instead of waiting for the next probe interval.
   *
   * No-op if the session has no circuit breaker or it is not open.
   */
  triggerProbe(sessionId: string): void {
    const cb = this._circuitBreakers.get(sessionId);
    if (!cb?.open) return;

    const wakeup = this._probeWakeups.get(sessionId);
    if (wakeup) {
      wakeup();
    }
  }

  /** @internal — visible for testing. */
  async _consumeStream(
    sessionId: string,
    signal: AbortSignal,
    onData: () => void,
  ): Promise<void> {
    let delay = STREAM_RECONNECT_BASE_MS;

    // Ensure circuit breaker state exists for this session.
    if (!this._circuitBreakers.has(sessionId)) {
      this._circuitBreakers.set(sessionId, { failures: 0, open: false });
    }
    const cb = this._circuitBreakers.get(sessionId)!;

    while (!signal.aborted) {
      try {
        const response = await fetch(
          `stream://localhost/terminal-output/${sessionId}`,
          { signal },
        );

        if (!response.ok || !response.body) {
          throw new Error(`Stream error: ${response.status}`);
        }

        // Successful connection — reset backoff and circuit breaker.
        delay = STREAM_RECONNECT_BASE_MS;
        if (cb.open) {
          console.info(
            `[TerminalService] Circuit breaker CLOSED for ${sessionId}, stream recovered`,
          );
        }
        cb.failures = 0;
        cb.open = false;

        const reader = response.body.getReader();
        // Cancel the reader when abort fires so reader.read() resolves
        // instead of hanging forever on a long-lived stream.
        const onAbort = () => reader.cancel();
        signal.addEventListener('abort', onAbort, { once: true });
        try {
          while (!signal.aborted) {
            const { done, value } = await reader.read();
            if (done) break;
            if (value && value.length > 0) {
              onData();
            }
          }
        } finally {
          signal.removeEventListener('abort', onAbort);
          reader.releaseLock();
        }
      } catch (err: unknown) {
        if (signal.aborted) break;

        cb.failures++;

        // Check if we should open the circuit breaker.
        if (!cb.open && cb.failures >= CIRCUIT_BREAKER_THRESHOLD) {
          cb.open = true;
          console.warn(
            `[TerminalService] Circuit breaker OPEN for ${sessionId} after ${cb.failures} failures`,
          );
        }

        console.debug(
          `[TerminalService] Output stream error for ${sessionId}, ` +
          `failures=${cb.failures}, open=${cb.open}, reconnecting in ~${cb.open ? CIRCUIT_BREAKER_PROBE_INTERVAL_MS : delay}ms (+ jitter)`,
          err instanceof Error ? err.message : err,
        );
      }

      // Always wait before reconnecting, whether stream ended cleanly or
      // with an error. This prevents tight reconnect loops if the server
      // is restarting or the session was closed.
      if (signal.aborted) break;

      // In open state, use the probe interval (and support wakeup).
      // In closed state, use exponential backoff with random jitter to break
      // thundering herd when all streams fail simultaneously.
      const baseWaitTime = cb.open ? CIRCUIT_BREAKER_PROBE_INTERVAL_MS : delay;
      const waitTime = baseWaitTime + Math.floor(jitterRng() * STREAM_RECONNECT_BASE_MS);

      await new Promise<void>((resolve) => {
        const timer = setTimeout(resolve, waitTime);
        const cleanup = () => { clearTimeout(timer); resolve(); };
        signal.addEventListener('abort', cleanup, { once: true });

        // If circuit breaker is open, allow triggerProbe() to wake us up early.
        if (cb.open) {
          this._probeWakeups.set(sessionId, () => {
            signal.removeEventListener('abort', cleanup);
            cleanup();
          });
        }
      });

      // Clean up probe wakeup after wait resolves.
      this._probeWakeups.delete(sessionId);

      if (!cb.open) {
        delay = Math.min(delay * 2, STREAM_RECONNECT_MAX_MS);
      }
    }

    // Clean up circuit breaker state when stream loop exits.
    this._circuitBreakers.delete(sessionId);
    this._probeWakeups.delete(sessionId);
  }

  destroy() {
    // Disconnect all active streams.
    for (const [sessionId] of this.streamControllers) {
      this.disconnectOutputStream(sessionId);
    }
    for (const [sessionId] of this.diffStreamControllers) {
      this.disconnectDiffStream(sessionId);
    }
    this.unlistenFns.forEach(fn => fn());
    this.outputListeners.clear();
    this.gridDiffListeners.clear();
  }
}

export const terminalService = new TerminalService();
