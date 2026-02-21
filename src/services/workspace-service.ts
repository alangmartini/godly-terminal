import { invoke } from '@tauri-apps/api/core';
import { store, Workspace, ShellType } from '../state/store';
import { terminalSettingsStore } from '../state/terminal-settings-store';
import { notificationStore } from '../state/notification-store';

export interface WorkspaceData {
  id: string;
  name: string;
  folder_path: string;
  tab_order: string[];
  shell_type?: 'windows' | 'pwsh' | 'cmd' | { wsl: { distribution: string | null } } | { custom: { program: string; args: string[] | null } };
  worktree_mode?: boolean;
  claude_code_mode?: boolean;
}

export interface WorktreeInfo {
  path: string;
  branch: string;
  commit: string;
  is_main: boolean;
}

class WorkspaceService {
  async createWorkspace(
    name: string,
    folderPath: string,
    shellType: ShellType = terminalSettingsStore.getDefaultShell()
  ): Promise<string> {
    // Convert frontend ShellType to backend format
    let backendShellType: WorkspaceData['shell_type'];
    if (shellType.type === 'wsl') {
      backendShellType = { wsl: { distribution: (shellType as { type: 'wsl'; distribution?: string }).distribution ?? null } };
    } else if (shellType.type === 'custom') {
      const custom = shellType as { type: 'custom'; program: string; args?: string[] };
      backendShellType = { custom: { program: custom.program, args: custom.args ?? null } };
    } else {
      backendShellType = shellType.type;
    }

    const workspaceId = await invoke<string>('create_workspace', {
      name,
      folderPath,
      shellType: backendShellType,
    });

    const workspace: Workspace = {
      id: workspaceId,
      name,
      folderPath,
      tabOrder: [],
      shellType,
      worktreeMode: false,
      claudeCodeMode: false,
    };
    store.addWorkspace(workspace);

    // Auto-mute if workspace name matches any glob pattern
    if (!notificationStore.isWorkspaceNotificationEnabled(workspaceId, name)) {
      notificationStore.setWorkspaceOverride(workspaceId, false);
    }

    return workspaceId;
  }

  async getWslDistributions(): Promise<string[]> {
    return invoke<string[]>('get_wsl_distributions');
  }

  async isWslAvailable(): Promise<boolean> {
    return invoke<boolean>('is_wsl_available');
  }

  async deleteWorkspace(workspaceId: string): Promise<void> {
    await invoke('delete_workspace', { workspaceId });
    store.removeWorkspace(workspaceId);
    notificationStore.cleanupWorkspaceOverride(workspaceId);
  }

  async moveTabToWorkspace(
    terminalId: string,
    targetWorkspaceId: string
  ): Promise<void> {
    await invoke('move_tab_to_workspace', {
      terminalId,
      targetWorkspaceId,
    });
    store.moveTerminalToWorkspace(terminalId, targetWorkspaceId);
  }

  async reorderTabs(workspaceId: string, tabOrder: string[]): Promise<void> {
    await invoke('reorder_tabs', {
      workspaceId,
      tabOrder,
    });
    store.reorderTerminals(workspaceId, tabOrder);
  }

  async loadWorkspaces(): Promise<Workspace[]> {
    const workspaces = await invoke<WorkspaceData[]>('get_workspaces');
    return workspaces.map(w => ({
      id: w.id,
      name: w.name,
      folderPath: w.folder_path,
      tabOrder: w.tab_order,
      shellType: this.convertShellType(w.shell_type),
      worktreeMode: w.worktree_mode ?? false,
      claudeCodeMode: w.claude_code_mode ?? false,
    }));
  }

  async toggleWorktreeMode(workspaceId: string, enabled: boolean): Promise<void> {
    await invoke('toggle_worktree_mode', { workspaceId, enabled });
    store.updateWorkspace(workspaceId, { worktreeMode: enabled });
  }

  async toggleClaudeCodeMode(workspaceId: string, enabled: boolean): Promise<void> {
    await invoke('toggle_claude_code_mode', { workspaceId, enabled });
    store.updateWorkspace(workspaceId, { claudeCodeMode: enabled });
  }

  async isGitRepo(folderPath: string): Promise<boolean> {
    return invoke<boolean>('is_git_repo', { folderPath });
  }

  async listWorktrees(folderPath: string): Promise<WorktreeInfo[]> {
    return invoke<WorktreeInfo[]>('list_worktrees', { folderPath });
  }

  async removeWorktree(
    folderPath: string,
    worktreePath: string,
    force?: boolean
  ): Promise<void> {
    await invoke('remove_worktree', { folderPath, worktreePath, force: force ?? false });
  }

  async cleanupAllWorktrees(folderPath: string): Promise<number> {
    return invoke<number>('cleanup_all_worktrees', { folderPath });
  }

  private convertShellType(
    backendType?: WorkspaceData['shell_type']
  ): ShellType {
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
}

export const workspaceService = new WorkspaceService();
