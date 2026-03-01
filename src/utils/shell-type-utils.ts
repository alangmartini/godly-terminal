import { store, ShellType } from '../state/store';
import { notificationStore } from '../state/notification-store';
import { getDisplayName } from '../components/TabBar';

export type BackendShellType =
  | 'windows'
  | 'pwsh'
  | 'cmd'
  | { wsl: { distribution: string | null } }
  | { custom: { program: string; args: string[] | null } };

export function convertShellType(backendType?: BackendShellType): ShellType {
  if (!backendType || backendType === 'windows') return { type: 'windows' };
  if (backendType === 'pwsh') return { type: 'pwsh' };
  if (backendType === 'cmd') return { type: 'cmd' };
  if (typeof backendType === 'object' && 'wsl' in backendType) {
    return {
      type: 'wsl',
      distribution: backendType.wsl.distribution ?? undefined,
    };
  }
  if (typeof backendType === 'object' && 'custom' in backendType) {
    return {
      type: 'custom',
      program: backendType.custom.program,
      args: backendType.custom.args ?? undefined,
    };
  }
  return { type: 'windows' };
}

export function shellTypeToProcessName(shellType: ShellType): string {
  switch (shellType.type) {
    case 'windows': return 'powershell';
    case 'pwsh': return 'pwsh';
    case 'cmd': return 'cmd';
    case 'wsl': return shellType.distribution ?? 'wsl';
    case 'custom': {
      const name = shellType.program.replace(/\\/g, '/').split('/').pop() ?? shellType.program;
      return name.replace(/\.exe$/i, '') || shellType.program;
    }
  }
}

export function buildNotificationTitle(terminalId: string): string {
  const state = store.getState();
  const terminal = state.terminals.find(t => t.id === terminalId);
  if (!terminal) return 'Godly Terminal';
  const workspace = state.workspaces.find(w => w.id === terminal.workspaceId);
  const terminalName = getDisplayName(terminal);
  return workspace ? `${workspace.name} › ${terminalName}` : terminalName;
}

/** Check if notifications for a terminal's workspace are suppressed. */
export function isWorkspaceNotificationSuppressed(terminalId: string): boolean {
  const state = store.getState();
  const terminal = state.terminals.find(t => t.id === terminalId);
  if (!terminal) return false;
  const workspace = state.workspaces.find(w => w.id === terminal.workspaceId);
  if (!workspace) return false;
  return !notificationStore.isWorkspaceNotificationEnabled(workspace.id, workspace.name);
}
