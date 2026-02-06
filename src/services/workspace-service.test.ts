import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';

// Mock the @tauri-apps/api/core module
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

// Import after mock setup
import { invoke } from '@tauri-apps/api/core';
import { workspaceService, WorkspaceData } from './workspace-service';

const mockedInvoke = vi.mocked(invoke);

describe('WorkspaceService', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store state
    store.setState({
      workspaces: [],
      terminals: [],
      activeWorkspaceId: null,
      activeTerminalId: null,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('convertShellType', () => {
    // We need to test the private method indirectly through loadWorkspaces
    it('should convert undefined to windows shell type', async () => {
      const workspaceData: WorkspaceData[] = [
        {
          id: 'ws-1',
          name: 'Test',
          folder_path: 'C:\\Users\\test',
          tab_order: [],
          shell_type: undefined,
        },
      ];

      mockedInvoke.mockResolvedValue(workspaceData);

      const workspaces = await workspaceService.loadWorkspaces();

      expect(workspaces[0].shellType).toEqual({ type: 'windows' });
    });

    it('should convert "windows" string to windows shell type', async () => {
      const workspaceData: WorkspaceData[] = [
        {
          id: 'ws-1',
          name: 'Test',
          folder_path: 'C:\\Users\\test',
          tab_order: [],
          shell_type: 'windows',
        },
      ];

      mockedInvoke.mockResolvedValue(workspaceData);

      const workspaces = await workspaceService.loadWorkspaces();

      expect(workspaces[0].shellType).toEqual({ type: 'windows' });
    });

    it('should convert { wsl: { distribution: "Ubuntu" } } to wsl with distribution', async () => {
      const workspaceData: WorkspaceData[] = [
        {
          id: 'ws-1',
          name: 'WSL Test',
          folder_path: '/home/user',
          tab_order: [],
          shell_type: { wsl: { distribution: 'Ubuntu' } },
        },
      ];

      mockedInvoke.mockResolvedValue(workspaceData);

      const workspaces = await workspaceService.loadWorkspaces();

      expect(workspaces[0].shellType).toEqual({
        type: 'wsl',
        distribution: 'Ubuntu',
      });
    });

    it('should convert { wsl: { distribution: null } } to wsl without distribution', async () => {
      const workspaceData: WorkspaceData[] = [
        {
          id: 'ws-1',
          name: 'WSL Default',
          folder_path: '/home/user',
          tab_order: [],
          shell_type: { wsl: { distribution: null } },
        },
      ];

      mockedInvoke.mockResolvedValue(workspaceData);

      const workspaces = await workspaceService.loadWorkspaces();

      expect(workspaces[0].shellType.type).toBe('wsl');
      if (workspaces[0].shellType.type === 'wsl') {
        expect(workspaces[0].shellType.distribution).toBeUndefined();
      }
    });
  });

  describe('createWorkspace', () => {
    it('should send windows shell type in correct backend format', async () => {
      mockedInvoke.mockResolvedValue('ws-new');

      await workspaceService.createWorkspace('New Workspace', 'C:\\test', {
        type: 'windows',
      });

      expect(mockedInvoke).toHaveBeenCalledWith('create_workspace', {
        name: 'New Workspace',
        folderPath: 'C:\\test',
        shellType: 'windows',
      });
    });

    it('should send wsl shell type with distribution in correct backend format', async () => {
      mockedInvoke.mockResolvedValue('ws-new');

      await workspaceService.createWorkspace('WSL Workspace', '/home/user', {
        type: 'wsl',
        distribution: 'Ubuntu-22.04',
      });

      expect(mockedInvoke).toHaveBeenCalledWith('create_workspace', {
        name: 'WSL Workspace',
        folderPath: '/home/user',
        shellType: { wsl: { distribution: 'Ubuntu-22.04' } },
      });
    });

    it('should send wsl shell type without distribution as null', async () => {
      mockedInvoke.mockResolvedValue('ws-new');

      await workspaceService.createWorkspace('WSL Default', '/home/user', {
        type: 'wsl',
      });

      expect(mockedInvoke).toHaveBeenCalledWith('create_workspace', {
        name: 'WSL Default',
        folderPath: '/home/user',
        shellType: { wsl: { distribution: null } },
      });
    });

    it('should default to windows shell type when not specified', async () => {
      mockedInvoke.mockResolvedValue('ws-new');

      await workspaceService.createWorkspace('Default', 'C:\\default');

      expect(mockedInvoke).toHaveBeenCalledWith('create_workspace', {
        name: 'Default',
        folderPath: 'C:\\default',
        shellType: 'windows',
      });
    });

    it('should add workspace to store after creation', async () => {
      mockedInvoke.mockResolvedValue('ws-created');

      await workspaceService.createWorkspace('Store Test', 'C:\\store', {
        type: 'wsl',
        distribution: 'Debian',
      });

      const state = store.getState();
      expect(state.workspaces).toHaveLength(1);
      expect(state.workspaces[0].id).toBe('ws-created');
      expect(state.workspaces[0].shellType).toEqual({
        type: 'wsl',
        distribution: 'Debian',
      });
    });
  });

  describe('getWslDistributions', () => {
    it('should return array of distributions from backend', async () => {
      mockedInvoke.mockResolvedValue(['Ubuntu', 'Debian', 'Alpine']);

      const distributions = await workspaceService.getWslDistributions();

      expect(mockedInvoke).toHaveBeenCalledWith('get_wsl_distributions');
      expect(distributions).toEqual(['Ubuntu', 'Debian', 'Alpine']);
    });

    it('should return empty array when no distributions', async () => {
      mockedInvoke.mockResolvedValue([]);

      const distributions = await workspaceService.getWslDistributions();

      expect(distributions).toEqual([]);
    });
  });

  describe('isWslAvailable', () => {
    it('should return true when WSL is available', async () => {
      mockedInvoke.mockResolvedValue(true);

      const available = await workspaceService.isWslAvailable();

      expect(mockedInvoke).toHaveBeenCalledWith('is_wsl_available');
      expect(available).toBe(true);
    });

    it('should return false when WSL is not available', async () => {
      mockedInvoke.mockResolvedValue(false);

      const available = await workspaceService.isWslAvailable();

      expect(available).toBe(false);
    });
  });

  describe('loadWorkspaces', () => {
    it('should load and convert multiple workspaces', async () => {
      const workspaceData: WorkspaceData[] = [
        {
          id: 'ws-1',
          name: 'Windows Workspace',
          folder_path: 'C:\\Users\\test',
          tab_order: ['term-1'],
          shell_type: 'windows',
        },
        {
          id: 'ws-2',
          name: 'WSL Workspace',
          folder_path: '/home/user',
          tab_order: ['term-2', 'term-3'],
          shell_type: { wsl: { distribution: 'Ubuntu' } },
        },
      ];

      mockedInvoke.mockResolvedValue(workspaceData);

      const workspaces = await workspaceService.loadWorkspaces();

      expect(workspaces).toHaveLength(2);

      expect(workspaces[0].id).toBe('ws-1');
      expect(workspaces[0].folderPath).toBe('C:\\Users\\test');
      expect(workspaces[0].shellType).toEqual({ type: 'windows' });

      expect(workspaces[1].id).toBe('ws-2');
      expect(workspaces[1].folderPath).toBe('/home/user');
      expect(workspaces[1].shellType).toEqual({
        type: 'wsl',
        distribution: 'Ubuntu',
      });
    });
  });

  describe('deleteWorkspace', () => {
    it('should remove workspace from store after deletion', async () => {
      mockedInvoke.mockResolvedValue(undefined);

      // First add a workspace
      store.addWorkspace({
        id: 'ws-to-delete',
        name: 'To Delete',
        folderPath: 'C:\\temp',
        tabOrder: [],
        shellType: { type: 'windows' },
      });

      expect(store.getState().workspaces).toHaveLength(1);

      await workspaceService.deleteWorkspace('ws-to-delete');

      expect(mockedInvoke).toHaveBeenCalledWith('delete_workspace', {
        workspaceId: 'ws-to-delete',
      });
      expect(store.getState().workspaces).toHaveLength(0);
    });
  });
});
