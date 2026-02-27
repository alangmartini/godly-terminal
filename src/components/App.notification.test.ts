import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';

// Mock Tauri APIs before importing anything that uses them
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

vi.mock('@tauri-apps/api/path', () => ({
  homeDir: vi.fn(() => Promise.resolve('C:\\Users\\test')),
}));

// Import after mock setup
import { buildNotificationTitle } from './App';

describe('Notification improvements', () => {
  beforeEach(() => {
    vi.clearAllMocks();
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

  describe('buildNotificationTitle', () => {
    it('returns "WorkspaceName › TerminalName" when both exist', () => {
      store.addWorkspace({
        id: 'ws-1',
        name: 'MyProject',
        folderPath: 'C:\\Projects',
        tabOrder: [],
        shellType: { type: 'windows' },
        worktreeMode: false,
        claudeCodeMode: false,
      });
      store.addTerminal({
        id: 'term-1',
        workspaceId: 'ws-1',
        name: 'bash',
        processName: 'bash',
        order: 0,
      });

      expect(buildNotificationTitle('term-1')).toBe('MyProject › bash');
    });

    it('falls back to terminal name when workspace not found', () => {
      // Add terminal with a workspaceId that has no matching workspace
      store.addWorkspace({
        id: 'ws-1',
        name: 'Workspace',
        folderPath: 'C:\\',
        tabOrder: [],
        shellType: { type: 'windows' },
        worktreeMode: false,
        claudeCodeMode: false,
      });
      store.addTerminal({
        id: 'term-orphan',
        workspaceId: 'ws-missing',
        name: 'powershell',
        processName: 'powershell',
        order: 0,
      });

      expect(buildNotificationTitle('term-orphan')).toBe('powershell');
    });

    it('falls back to "Godly Terminal" when terminal not found', () => {
      expect(buildNotificationTitle('nonexistent-id')).toBe('Godly Terminal');
    });

    it('uses oscTitle when terminal has one and is not user-renamed', () => {
      store.addWorkspace({
        id: 'ws-1',
        name: 'Dev',
        folderPath: 'C:\\',
        tabOrder: [],
        shellType: { type: 'windows' },
        worktreeMode: false,
        claudeCodeMode: false,
      });
      store.addTerminal({
        id: 'term-osc',
        workspaceId: 'ws-1',
        name: 'Terminal',
        processName: 'bash',
        order: 0,
        oscTitle: 'vim main.rs',
      });

      // getDisplayName prefers oscTitle over name when not user-renamed
      expect(buildNotificationTitle('term-osc')).toBe('Dev › vim main.rs');
    });

    it('uses user-renamed name over oscTitle', () => {
      store.addWorkspace({
        id: 'ws-1',
        name: 'Dev',
        folderPath: 'C:\\',
        tabOrder: [],
        shellType: { type: 'windows' },
        worktreeMode: false,
        claudeCodeMode: false,
      });
      store.addTerminal({
        id: 'term-renamed',
        workspaceId: 'ws-1',
        name: 'My Custom Name',
        processName: 'bash',
        order: 0,
        oscTitle: 'vim main.rs',
        userRenamed: true,
      });

      expect(buildNotificationTitle('term-renamed')).toBe('Dev › My Custom Name');
    });
  });
});
