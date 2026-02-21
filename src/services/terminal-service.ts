import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { store, ShellType } from '../state/store';
import { perfTracer } from '../utils/PerfTracer';
import type { RichGridDiff } from '../components/TerminalRenderer';


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
const STREAM_RECONNECT_BASE_MS = 1000;
/** Maximum delay between reconnect attempts (ms). */
const STREAM_RECONNECT_MAX_MS = 10_000;

class TerminalService {
  private outputListeners: Map<string, () => void> = new Map();
  private gridDiffListeners: Map<string, (diff: RichGridDiff) => void> = new Map();
  private unlistenFns: UnlistenFn[] = [];
  /** AbortControllers for active stream connections (keyed by session ID). */
  private streamControllers: Map<string, AbortController> = new Map();

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
  }

  /** @internal — visible for testing. */
  async _consumeStream(
    sessionId: string,
    signal: AbortSignal,
    onData: () => void,
  ): Promise<void> {
    let delay = STREAM_RECONNECT_BASE_MS;

    while (!signal.aborted) {
      try {
        const response = await fetch(
          `stream://localhost/terminal-output/${sessionId}`,
          { signal },
        );

        if (!response.ok || !response.body) {
          throw new Error(`Stream error: ${response.status}`);
        }

        // Successful connection — reset backoff.
        delay = STREAM_RECONNECT_BASE_MS;

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

        console.debug(
          `[TerminalService] Output stream error for ${sessionId}, reconnecting in ${delay}ms`,
          err instanceof Error ? err.message : err,
        );
      }

      // Always wait before reconnecting, whether stream ended cleanly or
      // with an error. This prevents tight reconnect loops if the server
      // is restarting or the session was closed.
      if (signal.aborted) break;

      await new Promise<void>((resolve) => {
        const timer = setTimeout(resolve, delay);
        signal.addEventListener('abort', () => { clearTimeout(timer); resolve(); }, { once: true });
      });

      delay = Math.min(delay * 2, STREAM_RECONNECT_MAX_MS);
    }
  }

  destroy() {
    // Disconnect all active streams.
    for (const [sessionId] of this.streamControllers) {
      this.disconnectOutputStream(sessionId);
    }
    this.unlistenFns.forEach(fn => fn());
    this.outputListeners.clear();
    this.gridDiffListeners.clear();
  }
}

export const terminalService = new TerminalService();
