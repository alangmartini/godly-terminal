import { invoke } from '@tauri-apps/api/core';
import { store, Workspace } from '../state/store';

export interface WorkspaceData {
  id: string;
  name: string;
  folder_path: string;
  tab_order: string[];
}

class WorkspaceService {
  async createWorkspace(name: string, folderPath: string): Promise<string> {
    const workspaceId = await invoke<string>('create_workspace', {
      name,
      folderPath,
    });

    const workspace: Workspace = {
      id: workspaceId,
      name,
      folderPath,
      tabOrder: [],
    };
    store.addWorkspace(workspace);

    return workspaceId;
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
    }));
  }
}

export const workspaceService = new WorkspaceService();
