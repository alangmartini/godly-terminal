import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';
import { store, ShellType } from '../state/store';

// Backend shell type format - matches Rust serde externally tagged enum
export type BackendShellType =
  | 'windows'
  | { wsl: { distribution: string | null } };

// Convert frontend ShellType to backend format
function toBackendShellType(shellType: ShellType): BackendShellType {
  if (shellType.type === 'windows') {
    return 'windows';
  }
  return { wsl: { distribution: shellType.distribution ?? null } };
}

export interface TerminalOutputPayload {
  terminal_id: string;
  data: number[];
}

export interface ProcessChangedPayload {
  terminal_id: string;
  process_name: string;
}

export interface TerminalClosedPayload {
  terminal_id: string;
}

class TerminalService {
  private outputListeners: Map<string, (data: Uint8Array) => void> = new Map();
  private unlistenFns: UnlistenFn[] = [];

  async init() {
    const unlistenOutput = await listen<TerminalOutputPayload>(
      'terminal-output',
      (event) => {
        const { terminal_id, data } = event.payload;
        const listener = this.outputListeners.get(terminal_id);
        if (listener) {
          listener(new Uint8Array(data));
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
        const { terminal_id } = event.payload;
        this.outputListeners.delete(terminal_id);
        store.removeTerminal(terminal_id);
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
    }
  ): Promise<string> {
    const terminalId = await invoke<string>('create_terminal', {
      workspaceId,
      cwdOverride: options?.cwdOverride ?? null,
      shellTypeOverride: options?.shellTypeOverride
        ? toBackendShellType(options.shellTypeOverride)
        : null,
      idOverride: options?.idOverride ?? null,
    });
    return terminalId;
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

  onTerminalOutput(terminalId: string, callback: (data: Uint8Array) => void) {
    this.outputListeners.set(terminalId, callback);
    return () => this.outputListeners.delete(terminalId);
  }

  destroy() {
    this.unlistenFns.forEach(fn => fn());
    this.outputListeners.clear();
  }
}

export const terminalService = new TerminalService();
