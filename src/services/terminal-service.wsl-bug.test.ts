import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';

// Mock the @tauri-apps/api modules
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(() => Promise.resolve(() => {})),
}));

// Import after mock setup
import { invoke } from '@tauri-apps/api/core';
import { terminalService } from './terminal-service';
import { terminalSettingsStore } from '../state/terminal-settings-store';

const mockedInvoke = vi.mocked(invoke);

/**
 * Bug: WSL shell type ignored when creating terminal in WSL workspace
 *
 * When a workspace is configured with WSL shell type, creating a terminal
 * without an explicit shellTypeOverride should use the workspace's shell type.
 * Instead, terminal-service.ts always fills in the global default shell
 * (PowerShell) via terminalSettingsStore.getDefaultShell(), which overrides
 * the workspace setting on the backend.
 *
 * Root cause: terminal-service.ts:102-103
 *   const shellOverride = options?.shellTypeOverride
 *     ?? terminalSettingsStore.getDefaultShell();
 *
 * This means shellTypeOverride is NEVER null — it always resolves to a value.
 * The Rust backend (terminal.rs:108-110) only falls through to workspace
 * shell_type when shell_type_override is None, which never happens.
 */
describe('WSL workspace shell type bug', () => {
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

  it('should NOT send a shellTypeOverride when caller does not provide one', async () => {
    // Bug: createTerminal always sends a shellTypeOverride (the global default),
    // so the workspace's WSL shell type is never used by the backend.
    //
    // Expected: shellTypeOverride should be null when no explicit override is given,
    // allowing the backend to use the workspace's shell type.
    mockedInvoke.mockResolvedValue({ id: 'term-1', worktree_branch: null });

    await terminalService.createTerminal('ws-wsl');

    expect(mockedInvoke).toHaveBeenCalledWith('create_terminal', expect.objectContaining({
      workspaceId: 'ws-wsl',
      shellTypeOverride: null,
    }));
  });

  it('should NOT send a shellTypeOverride when options are provided but shellTypeOverride is absent', async () => {
    // Bug: Even when the caller passes other options (like worktreeName) without
    // shellTypeOverride, the service fills in the global default shell.
    mockedInvoke.mockResolvedValue({ id: 'term-2', worktree_branch: null });

    await terminalService.createTerminal('ws-wsl', { worktreeName: 'my-feature' });

    expect(mockedInvoke).toHaveBeenCalledWith('create_terminal', expect.objectContaining({
      workspaceId: 'ws-wsl',
      shellTypeOverride: null,
    }));
  });

  it('should send the explicit shellTypeOverride when caller provides one', async () => {
    // When an explicit override IS provided, it should be sent through.
    // This case should still work correctly.
    mockedInvoke.mockResolvedValue({ id: 'term-3', worktree_branch: null });

    await terminalService.createTerminal('ws-windows', {
      shellTypeOverride: { type: 'wsl', distribution: 'Ubuntu' },
    });

    expect(mockedInvoke).toHaveBeenCalledWith('create_terminal', expect.objectContaining({
      workspaceId: 'ws-windows',
      shellTypeOverride: { wsl: { distribution: 'Ubuntu' } },
    }));
  });

  it('should not override workspace WSL shell with global default PowerShell', async () => {
    // Bug: The global default is { type: 'windows' }, and it gets sent as
    // shellTypeOverride even for WSL workspaces. This test verifies that
    // the global default is NOT sent when the caller doesn't explicitly override.
    mockedInvoke.mockResolvedValue({ id: 'term-4', worktree_branch: null });

    // Ensure global default is windows (PowerShell) — this is the default
    expect(terminalSettingsStore.getDefaultShell()).toEqual({ type: 'windows' });

    // Create terminal without explicit shell type
    await terminalService.createTerminal('ws-wsl-workspace');

    // The invoke should NOT contain a windows shellTypeOverride
    const invokeCall = mockedInvoke.mock.calls.find(c => c[0] === 'create_terminal');
    expect(invokeCall).toBeDefined();
    const args = invokeCall![1] as Record<string, unknown>;
    expect(args.shellTypeOverride).not.toBe('windows');
    expect(args.shellTypeOverride).toBeNull();
  });

  it('should not override workspace WSL shell when global default is also WSL but different distro', async () => {
    // Edge case: Even when the global default is WSL with a different distro,
    // the workspace shell type should take precedence (via the backend), not
    // the global default.
    mockedInvoke.mockResolvedValue({ id: 'term-5', worktree_branch: null });

    // Set global default to WSL with Ubuntu
    terminalSettingsStore.setDefaultShell({ type: 'wsl', distribution: 'Ubuntu' });

    // Create terminal for a workspace that uses Debian — no explicit override
    await terminalService.createTerminal('ws-debian-workspace');

    // Should NOT send any shellTypeOverride (let the workspace's Debian setting win)
    const invokeCall = mockedInvoke.mock.calls.find(c => c[0] === 'create_terminal');
    expect(invokeCall).toBeDefined();
    const args = invokeCall![1] as Record<string, unknown>;
    expect(args.shellTypeOverride).toBeNull();

    // Clean up
    terminalSettingsStore.setDefaultShell({ type: 'windows' });
  });
});
