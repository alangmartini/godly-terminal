import { invoke } from '@tauri-apps/api/core';
import { store, Workspace, ShellType } from '../state/store';

export interface WorkspaceData {
  id: string;
  name: string;
  folder_path: string;
  tab_order: string[];
  shell_type?: 'windows' | { wsl: { distribution: string | null } };
}

class WorkspaceService {
  async createWorkspace(
    name: string,
    folderPath: string,
    shellType: ShellType = { type: 'windows' }
  ): Promise<string> {
    // Convert frontend ShellType to backend format
    const backendShellType =
      shellType.type === 'windows'
        ? 'windows'
        : { wsl: { distribution: shellType.distribution ?? null } };

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
    };
    store.addWorkspace(workspace);

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
    }));
  }

  private convertShellType(
    backendType?: 'windows' | { wsl: { distribution: string | null } }
  ): ShellType {
    if (!backendType || backendType === 'windows') {
      return { type: 'windows' };
    }
    if (typeof backendType === 'object' && 'wsl' in backendType) {
      return {
        type: 'wsl',
        distribution: backendType.wsl.distribution ?? undefined,
      };
    }
    return { type: 'windows' };
  }
}

export const workspaceService = new WorkspaceService();
