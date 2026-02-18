import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { store, ShellType } from '../state/store';
import { perfTracer } from '../utils/PerfTracer';


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

export interface TerminalClosedPayload {
  terminal_id: string;
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

class TerminalService {
  private outputListeners: Map<string, () => void> = new Map();
  private unlistenFns: UnlistenFn[] = [];

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

    const unlistenProcess = await listen<ProcessChangedPayload>(
      'process-changed',
      (event) => {
        const { terminal_id, process_name } = event.payload;
        store.updateTerminal(terminal_id, { processName: process_name, oscTitle: undefined });
      }
    );

    const unlistenClosed = await listen<TerminalClosedPayload>(
      'terminal-closed',
      (event) => {
        const { terminal_id } = event.payload;
        this.outputListeners.delete(terminal_id);
        store.updateTerminal(terminal_id, { exited: true });
        // Free daemon session resources (fire-and-forget)
        invoke('close_terminal', { terminalId: terminal_id }).catch(() => {});
      }
    );

    this.unlistenFns.push(unlistenOutput, unlistenProcess, unlistenClosed);
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

  destroy() {
    this.unlistenFns.forEach(fn => fn());
    this.outputListeners.clear();
  }
}

export const terminalService = new TerminalService();
