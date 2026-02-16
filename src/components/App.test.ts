import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';

// Mock the @tauri-apps/api modules
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@tauri-apps/api/path', () => ({
  homeDir: vi.fn(() => Promise.resolve('C:\\Users\\test')),
}));

// Import after mock setup
import { invoke } from '@tauri-apps/api/core';
import { terminalService } from '../services/terminal-service';

const mockedInvoke = vi.mocked(invoke);

// Type for backend shell type format
type BackendShellType = 'windows' | 'pwsh' | 'cmd' | { wsl: { distribution: string | null } };

// Helper to convert shell type (mirrors App.ts logic)
function convertShellType(
  backendType?: BackendShellType
): { type: 'windows' } | { type: 'pwsh' } | { type: 'cmd' } | { type: 'wsl'; distribution?: string } {
  if (!backendType || backendType === 'windows') return { type: 'windows' };
  if (backendType === 'pwsh') return { type: 'pwsh' };
  if (backendType === 'cmd') return { type: 'cmd' };
  if (typeof backendType === 'object' && 'wsl' in backendType) {
    return {
      type: 'wsl',
      distribution: backendType.wsl.distribution ?? undefined,
    };
  }
  return { type: 'windows' };
}

describe('App Persistence', () => {
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

  describe('load_layout integration', () => {
    it('should correctly parse layout with workspaces and terminals', async () => {
      // This is what the backend returns
      const savedLayout = {
        workspaces: [
          {
            id: 'ws-123',
            name: 'My Workspace',
            folder_path: 'C:\\Projects',
            tab_order: ['term-456'],
            shell_type: 'windows',
          },
        ],
        terminals: [
          {
            id: 'term-456',
            workspace_id: 'ws-123',
            name: 'Terminal 1',
            shell_type: 'windows',
            cwd: 'C:\\Projects\\myapp',
          },
        ],
        active_workspace_id: 'ws-123',
      };

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'load_layout') {
          return savedLayout;
        }
        if (cmd === 'create_terminal') {
          // Return the same ID that was passed in (simulating id_override working)
          const typedArgs = args as { idOverride?: string };
          return typedArgs?.idOverride ?? 'new-id';
        }
        return undefined;
      });

      // Simulate what App.init() does
      const layout = await invoke<typeof savedLayout>('load_layout');

      expect(layout.workspaces.length).toBe(1);
      expect(layout.workspaces[0].id).toBe('ws-123');
      expect(layout.terminals.length).toBe(1);
      expect(layout.terminals[0].id).toBe('term-456');
    });

    it('should pass terminal ID to create_terminal when restoring', async () => {
      const savedLayout = {
        workspaces: [
          {
            id: 'ws-123',
            name: 'Test',
            folder_path: 'C:\\Test',
            tab_order: [],
            shell_type: 'windows',
          },
        ],
        terminals: [
          {
            id: 'original-term-id-12345',
            workspace_id: 'ws-123',
            name: 'Restored Terminal',
            shell_type: 'windows',
            cwd: 'C:\\Test',
          },
        ],
        active_workspace_id: 'ws-123',
      };

      const createTerminalCalls: unknown[] = [];

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'load_layout') {
          return savedLayout;
        }
        if (cmd === 'create_terminal') {
          createTerminalCalls.push(args);
          const typedArgs = args as { idOverride?: string };
          return typedArgs?.idOverride ?? 'fallback-id';
        }
        return undefined;
      });

      // Simulate the restore flow from App.init()
      const layout = await invoke<typeof savedLayout>('load_layout');

      for (const t of layout.terminals) {
        const terminalId = await invoke<string>('create_terminal', {
          workspaceId: t.workspace_id,
          cwdOverride: t.cwd ?? null,
          shellTypeOverride: null,
          idOverride: t.id, // This is the key fix!
        });

        // The returned ID should match what we passed in
        expect(terminalId).toBe('original-term-id-12345');
      }

      // Verify create_terminal was called with the original ID
      expect(createTerminalCalls).toHaveLength(1);
      expect(createTerminalCalls[0]).toMatchObject({
        workspaceId: 'ws-123',
        idOverride: 'original-term-id-12345',
      });
    });

    it('should handle empty layout by creating default workspace', async () => {
      const emptyLayout = {
        workspaces: [],
        terminals: [],
        active_workspace_id: null,
      };

      mockedInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'load_layout') {
          return emptyLayout;
        }
        if (cmd === 'create_workspace') {
          return 'default-ws-id';
        }
        if (cmd === 'create_terminal') {
          return 'default-term-id';
        }
        return undefined;
      });

      const layout = await invoke<typeof emptyLayout>('load_layout');

      expect(layout.workspaces.length).toBe(0);
      // App.init would call createDefaultWorkspace here
    });

    it('should handle WSL shell type in layout', async () => {
      const wslLayout = {
        workspaces: [
          {
            id: 'ws-wsl',
            name: 'WSL Workspace',
            folder_path: '/home/user',
            tab_order: [],
            shell_type: { wsl: { distribution: 'Ubuntu' } },
          },
        ],
        terminals: [
          {
            id: 'term-wsl',
            workspace_id: 'ws-wsl',
            name: 'WSL Terminal',
            shell_type: { wsl: { distribution: 'Ubuntu' } },
            cwd: '/home/user/projects',
          },
        ],
        active_workspace_id: 'ws-wsl',
      };

      mockedInvoke.mockResolvedValue(wslLayout);

      const layout = await invoke<typeof wslLayout>('load_layout');

      expect(layout.workspaces[0].shell_type).toEqual({
        wsl: { distribution: 'Ubuntu' },
      });
      expect(layout.terminals[0].shell_type).toEqual({
        wsl: { distribution: 'Ubuntu' },
      });
    });

    it('should handle pwsh shell type in layout', async () => {
      const pwshLayout = {
        workspaces: [
          {
            id: 'ws-pwsh',
            name: 'PowerShell 7 Workspace',
            folder_path: 'C:\\Projects',
            tab_order: [],
            shell_type: 'pwsh' as const,
          },
        ],
        terminals: [],
        active_workspace_id: 'ws-pwsh',
      };

      mockedInvoke.mockResolvedValue(pwshLayout);
      const layout = await invoke<typeof pwshLayout>('load_layout');

      const shellType = convertShellType(layout.workspaces[0].shell_type);
      expect(shellType).toEqual({ type: 'pwsh' });
    });

    it('should handle cmd shell type in layout', async () => {
      const cmdLayout = {
        workspaces: [
          {
            id: 'ws-cmd',
            name: 'Command Prompt Workspace',
            folder_path: 'C:\\Projects',
            tab_order: [],
            shell_type: 'cmd' as const,
          },
        ],
        terminals: [],
        active_workspace_id: 'ws-cmd',
      };

      mockedInvoke.mockResolvedValue(cmdLayout);
      const layout = await invoke<typeof cmdLayout>('load_layout');

      const shellType = convertShellType(layout.workspaces[0].shell_type);
      expect(shellType).toEqual({ type: 'cmd' });
    });
  });

  describe('claude code mode persistence', () => {
    it('should restore claudeCodeMode from layout', async () => {
      const savedLayout = {
        workspaces: [
          {
            id: 'ws-cc',
            name: 'CC Workspace',
            folder_path: 'C:\\Projects',
            tab_order: [],
            shell_type: 'windows',
            claude_code_mode: true,
          },
        ],
        terminals: [],
        active_workspace_id: 'ws-cc',
      };

      mockedInvoke.mockResolvedValue(savedLayout);

      const layout = await invoke<typeof savedLayout>('load_layout');

      // Simulate App.init() restore logic
      layout.workspaces.forEach((w) => {
        store.addWorkspace({
          id: w.id,
          name: w.name,
          folderPath: w.folder_path,
          tabOrder: w.tab_order,
          shellType: convertShellType(w.shell_type),
          worktreeMode: false,
          claudeCodeMode: w.claude_code_mode ?? false,
        });
      });

      const state = store.getState();
      expect(state.workspaces[0].claudeCodeMode).toBe(true);
    });

    it('should default claudeCodeMode to false when missing from old layout', async () => {
      // Simulate an old layout that doesn't have claude_code_mode field
      const oldLayout = {
        workspaces: [
          {
            id: 'ws-old',
            name: 'Old Workspace',
            folder_path: 'C:\\Old',
            tab_order: [],
            shell_type: 'windows',
            // no claude_code_mode field
          },
        ],
        terminals: [],
        active_workspace_id: 'ws-old',
      };

      mockedInvoke.mockResolvedValue(oldLayout);

      const layout = await invoke<typeof oldLayout>('load_layout');

      layout.workspaces.forEach((w) => {
        store.addWorkspace({
          id: w.id,
          name: w.name,
          folderPath: w.folder_path,
          tabOrder: w.tab_order,
          shellType: convertShellType(w.shell_type),
          worktreeMode: false,
          claudeCodeMode: (w as any).claude_code_mode ?? false,
        });
      });

      const state = store.getState();
      expect(state.workspaces[0].claudeCodeMode).toBe(false);
    });
  });

  describe('scrollback persistence', () => {
    it('should load scrollback using the same terminal ID', async () => {
      const terminalId = 'term-with-scrollback-123';
      const scrollbackData = [72, 101, 108, 108, 111]; // "Hello" as bytes

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'load_scrollback') {
          const typedArgs = args as { terminalId: string };
          if (typedArgs.terminalId === terminalId) {
            return scrollbackData;
          }
          return [];
        }
        return undefined;
      });

      const result = await invoke<number[]>('load_scrollback', { terminalId });

      expect(result).toEqual(scrollbackData);
    });

    it('should fail to load scrollback if terminal ID changes', async () => {
      // This simulates the bug: scrollback saved with old ID, trying to load with new ID
      const originalId = 'original-term-id';
      const newId = 'new-term-id-after-restart';
      const scrollbackData = [72, 101, 108, 108, 111];

      // Store scrollback under original ID
      const scrollbackStore = new Map<string, number[]>();
      scrollbackStore.set(originalId, scrollbackData);

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'load_scrollback') {
          const typedArgs = args as { terminalId: string };
          return scrollbackStore.get(typedArgs.terminalId) ?? [];
        }
        return undefined;
      });

      // Try to load with new ID (this was the bug)
      const result = await invoke<number[]>('load_scrollback', {
        terminalId: newId,
      });

      // Should be empty because the IDs don't match
      expect(result).toEqual([]);

      // But if we use the original ID, it works
      const correctResult = await invoke<number[]>('load_scrollback', {
        terminalId: originalId,
      });
      expect(correctResult).toEqual(scrollbackData);
    });
  });

  describe('full restore flow simulation', () => {
    it('should simulate complete App.init() restore flow', async () => {
      // This test simulates the exact flow in App.init()
      const savedLayout = {
        workspaces: [
          {
            id: 'ws-saved-123',
            name: 'Saved Workspace',
            folder_path: 'C:\\Projects',
            tab_order: ['term-saved-456'],
            shell_type: 'windows' as const,
          },
        ],
        terminals: [
          {
            id: 'term-saved-456',
            workspace_id: 'ws-saved-123',
            name: 'Saved Terminal',
            shell_type: 'windows' as const,
            cwd: 'C:\\Projects\\app',
          },
        ],
        active_workspace_id: 'ws-saved-123',
      };

      const createdTerminals: Array<{
        workspaceId: string;
        idOverride: string | null;
      }> = [];

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'load_layout') {
          return savedLayout;
        }
        if (cmd === 'create_terminal') {
          const typedArgs = args as {
            workspaceId: string;
            idOverride?: string | null;
          };
          createdTerminals.push({
            workspaceId: typedArgs.workspaceId,
            idOverride: typedArgs.idOverride ?? null,
          });
          // Return the idOverride if provided (simulating the fix)
          return typedArgs.idOverride ?? 'new-fallback-id';
        }
        return undefined;
      });

      // Simulate App.init() flow
      const layout = await invoke<typeof savedLayout>('load_layout');

      // Step 1: Restore workspaces to frontend store
      layout.workspaces.forEach((w) => {
        store.addWorkspace({
          id: w.id,
          name: w.name,
          folderPath: w.folder_path,
          tabOrder: w.tab_order,
          shellType: convertShellType(w.shell_type),
        });
      });

      // Step 2: Set active workspace
      store.setActiveWorkspace(layout.active_workspace_id || layout.workspaces[0].id);

      // Step 3: Restore terminals with original IDs
      for (const t of layout.terminals) {
        const terminalId = await invoke<string>('create_terminal', {
          workspaceId: t.workspace_id,
          cwdOverride: t.cwd ?? null,
          shellTypeOverride: null,
          idOverride: t.id, // KEY: Pass original ID
        });

        store.addTerminal({
          id: terminalId,
          workspaceId: t.workspace_id,
          name: t.name,
          processName: 'powershell',
          order: 0,
        });
      }

      // Verify the flow
      const state = store.getState();

      // Workspace was restored
      expect(state.workspaces).toHaveLength(1);
      expect(state.workspaces[0].id).toBe('ws-saved-123');

      // Active workspace was set
      expect(state.activeWorkspaceId).toBe('ws-saved-123');

      // Terminal was created with original ID
      expect(createdTerminals).toHaveLength(1);
      expect(createdTerminals[0].idOverride).toBe('term-saved-456');

      // Terminal in store has the preserved ID
      expect(state.terminals).toHaveLength(1);
      expect(state.terminals[0].id).toBe('term-saved-456');
    });

    it('should handle WSL shell type in restore flow', async () => {
      const wslLayout = {
        workspaces: [
          {
            id: 'ws-wsl',
            name: 'WSL Project',
            folder_path: '/home/user/project',
            tab_order: [],
            shell_type: { wsl: { distribution: 'Ubuntu-22.04' } } as const,
          },
        ],
        terminals: [
          {
            id: 'term-wsl',
            workspace_id: 'ws-wsl',
            name: 'Ubuntu Terminal',
            shell_type: { wsl: { distribution: 'Ubuntu-22.04' } } as const,
            cwd: '/home/user/project/src',
          },
        ],
        active_workspace_id: 'ws-wsl',
      };

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'load_layout') return wslLayout;
        if (cmd === 'create_terminal') {
          const typedArgs = args as { idOverride?: string };
          return typedArgs.idOverride ?? 'new-id';
        }
        return undefined;
      });

      const layout = await invoke<typeof wslLayout>('load_layout');

      // Convert and verify shell type
      const shellType = convertShellType(layout.workspaces[0].shell_type);
      expect(shellType.type).toBe('wsl');
      if (shellType.type === 'wsl') {
        expect(shellType.distribution).toBe('Ubuntu-22.04');
      }
    });
  });

  describe('terminalService.createTerminal with idOverride', () => {
    it('should pass idOverride to backend', async () => {
      let capturedArgs: unknown = null;

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'create_terminal') {
          capturedArgs = args;
          return { id: 'returned-id', worktree_branch: null };
        }
        return undefined;
      });

      const result = await terminalService.createTerminal('ws-1', {
        idOverride: 'my-custom-id',
      });

      expect(result.id).toBe('returned-id');
      expect(capturedArgs).toMatchObject({
        workspaceId: 'ws-1',
        idOverride: 'my-custom-id',
      });
    });

    it('should pass null for idOverride when not specified', async () => {
      let capturedArgs: unknown = null;

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'create_terminal') {
          capturedArgs = args;
          return { id: 'new-id', worktree_branch: null };
        }
        return undefined;
      });

      const result = await terminalService.createTerminal('ws-1');

      expect(result.id).toBe('new-id');
      expect(capturedArgs).toMatchObject({
        workspaceId: 'ws-1',
        idOverride: null,
      });
    });

    it('should pass worktreeName to backend', async () => {
      let capturedArgs: unknown = null;

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'create_terminal') {
          capturedArgs = args;
          return { id: 'wt-id', worktree_branch: 'wt-my-feature' };
        }
        return undefined;
      });

      const result = await terminalService.createTerminal('ws-1', {
        worktreeName: 'my-feature',
      });

      expect(result.id).toBe('wt-id');
      expect(result.worktree_branch).toBe('wt-my-feature');
      expect(capturedArgs).toMatchObject({
        workspaceId: 'ws-1',
        worktreeName: 'my-feature',
      });
    });
  });

  describe('tab name restoration', () => {
    it('should pass saved tab name to backend when restoring dead sessions', async () => {
      // Bug: create_terminal hardcodes name="Terminal" in the backend,
      // so after autosave the custom tab name is lost.
      const savedLayout = {
        workspaces: [
          {
            id: 'ws-1',
            name: 'Workspace',
            folder_path: 'C:\\Test',
            tab_order: ['term-1'],
            shell_type: 'windows' as const,
          },
        ],
        terminals: [
          {
            id: 'term-1',
            workspace_id: 'ws-1',
            name: 'My Custom Tab',
            shell_type: 'windows' as const,
            cwd: 'C:\\Test',
          },
        ],
        active_workspace_id: 'ws-1',
      };

      let createTerminalArgs: Record<string, unknown> | null = null;

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'load_layout') return savedLayout;
        if (cmd === 'reconnect_sessions') return []; // no live sessions
        if (cmd === 'create_terminal') {
          createTerminalArgs = args as Record<string, unknown>;
          return { id: 'term-1', worktree_branch: null };
        }
        return undefined;
      });

      // Simulate the restore flow from App.init()
      const layout = await invoke<typeof savedLayout>('load_layout');
      const liveSessions = await invoke<unknown[]>('reconnect_sessions');
      const liveSessionIds = new Set((liveSessions as Array<{ id: string }>).map(s => s.id));

      for (const t of layout.terminals) {
        const tabName = t.worktree_branch || t.name;

        if (!liveSessionIds.has(t.id)) {
          await terminalService.createTerminal(t.workspace_id, {
            cwdOverride: t.cwd ?? undefined,
            idOverride: t.id,
            nameOverride: tabName,
          });
        }
      }

      // Verify the saved tab name was passed to the backend
      expect(createTerminalArgs).not.toBeNull();
      expect(createTerminalArgs!['nameOverride']).toBe('My Custom Tab');
    });

    it('should pass worktree_branch as name when available', async () => {
      const savedLayout = {
        workspaces: [
          {
            id: 'ws-1',
            name: 'Workspace',
            folder_path: 'C:\\Test',
            tab_order: ['term-1'],
            shell_type: 'windows' as const,
          },
        ],
        terminals: [
          {
            id: 'term-1',
            workspace_id: 'ws-1',
            name: 'Terminal',
            shell_type: 'windows' as const,
            cwd: 'C:\\Test',
            worktree_branch: 'feat/my-branch',
          },
        ],
        active_workspace_id: 'ws-1',
      };

      let createTerminalArgs: Record<string, unknown> | null = null;

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        if (cmd === 'load_layout') return savedLayout;
        if (cmd === 'reconnect_sessions') return [];
        if (cmd === 'create_terminal') {
          createTerminalArgs = args as Record<string, unknown>;
          return { id: 'term-1', worktree_branch: 'feat/my-branch' };
        }
        return undefined;
      });

      const layout = await invoke<typeof savedLayout>('load_layout');
      const liveSessions = await invoke<unknown[]>('reconnect_sessions');
      const liveSessionIds = new Set((liveSessions as Array<{ id: string }>).map(s => s.id));

      for (const t of layout.terminals) {
        const tabName = t.worktree_branch || t.name;

        if (!liveSessionIds.has(t.id)) {
          await terminalService.createTerminal(t.workspace_id, {
            cwdOverride: t.cwd ?? undefined,
            idOverride: t.id,
            nameOverride: tabName,
          });
        }
      }

      expect(createTerminalArgs).not.toBeNull();
      expect(createTerminalArgs!['nameOverride']).toBe('feat/my-branch');
    });
  });

  describe('Terminal Persistence E2E - Full Cycle', () => {
    it('should preserve terminal path, scrollback size and bytes across restart', async () => {
      // === PHASE 1: CREATE ===
      const originalTerminalId = 'term-persist-e2e-123';
      const originalWorkspaceId = 'ws-persist-e2e-456';
      const originalCwd = 'C:\\Users\\test\\Projects\\myapp';
      const originalName = 'My Terminal';

      // Scrollback content with known size
      const scrollbackText =
        'PS C:\\Users\\test> echo "Hello World"\nHello World\n';
      const scrollbackBytes = new TextEncoder().encode(scrollbackText);
      const scrollbackSize = scrollbackBytes.length;

      // === PHASE 2: SAVE (simulate app saving before close) ===
      const savedScrollback = new Map<string, number[]>();
      let savedLayout: {
        workspaces: Array<{
          id: string;
          name: string;
          folder_path: string;
          tab_order: string[];
          shell_type: string;
        }>;
        terminals: Array<{
          id: string;
          workspace_id: string;
          name: string;
          shell_type: string;
          cwd: string;
        }>;
        active_workspace_id: string;
      } | null = null;

      // === PHASE 3: LOAD (simulate app restart) ===
      // Mock will return saved data

      mockedInvoke.mockImplementation(async (cmd: string, args?: unknown) => {
        // Save operations
        if (cmd === 'save_scrollback') {
          const { terminalId, data } = args as {
            terminalId: string;
            data: number[];
          };
          savedScrollback.set(terminalId, data);
          return undefined;
        }
        if (cmd === 'save_layout') {
          // Layout is already captured in savedLayout
          return undefined;
        }

        // Load operations (simulate restart)
        if (cmd === 'load_layout') {
          return savedLayout;
        }
        if (cmd === 'load_scrollback') {
          const { terminalId } = args as { terminalId: string };
          return savedScrollback.get(terminalId) ?? [];
        }
        if (cmd === 'create_terminal') {
          const { idOverride } = args as {
            idOverride?: string | null;
            cwdOverride?: string | null;
          };
          // Return original ID if provided (preserves scrollback linkage)
          return idOverride ?? 'new-fallback-id';
        }
        return undefined;
      });

      // --- Simulate SAVE phase ---
      // Save scrollback
      await invoke('save_scrollback', {
        terminalId: originalTerminalId,
        data: Array.from(scrollbackBytes),
      });

      // Prepare layout (what save_layout would produce)
      savedLayout = {
        workspaces: [
          {
            id: originalWorkspaceId,
            name: 'Test Workspace',
            folder_path: 'C:\\Users\\test',
            tab_order: [originalTerminalId],
            shell_type: 'windows',
          },
        ],
        terminals: [
          {
            id: originalTerminalId,
            workspace_id: originalWorkspaceId,
            name: originalName,
            shell_type: 'windows',
            cwd: originalCwd,
          },
        ],
        active_workspace_id: originalWorkspaceId,
      };

      // --- Simulate RESTART & LOAD phase ---
      // Reset store (fresh app start)
      store.setState({
        workspaces: [],
        terminals: [],
        activeWorkspaceId: null,
        activeTerminalId: null,
      });

      // Load layout
      const layout = await invoke<typeof savedLayout>('load_layout');

      // Restore workspace
      store.addWorkspace({
        id: layout!.workspaces[0].id,
        name: layout!.workspaces[0].name,
        folderPath: layout!.workspaces[0].folder_path,
        tabOrder: layout!.workspaces[0].tab_order,
        shellType: { type: 'windows' },
      });

      // Restore terminal with original ID and CWD
      const restoredTerminalId = await invoke<string>('create_terminal', {
        workspaceId: layout!.terminals[0].workspace_id,
        cwdOverride: layout!.terminals[0].cwd,
        shellTypeOverride: null,
        idOverride: layout!.terminals[0].id,
      });

      store.addTerminal({
        id: restoredTerminalId,
        workspaceId: layout!.terminals[0].workspace_id,
        name: layout!.terminals[0].name,
        processName: 'powershell',
        order: 0,
      });

      // Load scrollback
      const restoredScrollback = await invoke<number[]>('load_scrollback', {
        terminalId: restoredTerminalId,
      });

      // === PHASE 4: VERIFY ===
      const state = store.getState();

      // Verify terminal ID preserved
      expect(restoredTerminalId).toBe(originalTerminalId);
      expect(state.terminals[0].id).toBe(originalTerminalId);

      // Verify terminal CWD preserved (passed to create_terminal)
      expect(layout!.terminals[0].cwd).toBe(originalCwd);

      // Verify scrollback SIZE preserved
      expect(restoredScrollback.length).toBe(scrollbackSize);

      // Verify scrollback BYTES preserved (exact content)
      const restoredBytes = new Uint8Array(restoredScrollback);
      expect(restoredBytes).toEqual(scrollbackBytes);

      // Verify scrollback content as string
      const restoredText = new TextDecoder().decode(restoredBytes);
      expect(restoredText).toBe(scrollbackText);
    });
  });
});

