import { describe, it, expect, beforeEach } from 'vitest';
import { store, Workspace } from './store';

/**
 * Bug #204: MCP creates a second WebView window ("Godly Terminal - Agent") for
 * Agent-workspace terminals. app_handle.emit() broadcasts terminal-output events
 * to ALL windows. Under heavy output, both windows compete for resources, causing
 * a WebView2 crash (white screen → process death).
 *
 * Fix: Agent workspaces must be visible in the main window — no second window needed.
 * getVisibleWorkspaces() must include Agent workspaces in the main window.
 */
describe('Bug #204: MCP Agent workspace visibility', () => {
  const regularWorkspace: Workspace = {
    id: 'ws-regular',
    name: 'My Project',
    folderPath: 'C:\\Projects\\myapp',
    tabOrder: [],
    shellType: { type: 'windows' },
    worktreeMode: false,
    claudeCodeMode: false,
  };

  const agentWorkspace: Workspace = {
    id: 'ws-agent',
    name: 'Agent',
    folderPath: 'C:\\Projects\\myapp',
    tabOrder: [],
    shellType: { type: 'windows' },
    worktreeMode: false,
    claudeCodeMode: false,
  };

  beforeEach(() => {
    store.reset();
  });

  it('should include Agent workspace in getVisibleWorkspaces() for main window', () => {
    // Bug #204: Agent workspace was filtered out of main window, requiring a
    // second WebView window to display it. This caused crash under heavy output.
    store.addWorkspace(regularWorkspace);
    store.addWorkspace(agentWorkspace);

    const visible = store.getVisibleWorkspaces();

    // Agent workspace must be visible in the main window
    expect(visible.some(w => w.name === 'Agent')).toBe(true);
    expect(visible).toHaveLength(2);
  });

  it('should show Agent workspace terminals alongside regular workspaces', () => {
    // When MCP creates terminals, they go into the Agent workspace.
    // The main window must display this workspace so users can see MCP terminals.
    store.addWorkspace(regularWorkspace);
    store.addWorkspace(agentWorkspace);

    store.addTerminal({
      id: 'term-regular', workspaceId: 'ws-regular',
      name: 'PowerShell', processName: 'powershell.exe', order: 0,
    });
    store.addTerminal({
      id: 'term-agent', workspaceId: 'ws-agent',
      name: 'Claude Agent', processName: 'powershell.exe', order: 0,
    });

    const visible = store.getVisibleWorkspaces();
    const agentWs = visible.find(w => w.name === 'Agent');
    expect(agentWs).toBeDefined();

    // Agent workspace should have its terminals accessible
    const agentTerminals = store.getWorkspaceTerminals('ws-agent');
    expect(agentTerminals).toHaveLength(1);
    expect(agentTerminals[0].id).toBe('term-agent');
  });

  it('should still show regular workspaces when Agent workspace exists', () => {
    store.addWorkspace(regularWorkspace);
    store.addWorkspace(agentWorkspace);

    const visible = store.getVisibleWorkspaces();
    expect(visible.some(w => w.name === 'My Project')).toBe(true);
  });

  it('should work with no Agent workspace present', () => {
    store.addWorkspace(regularWorkspace);

    const visible = store.getVisibleWorkspaces();
    expect(visible).toHaveLength(1);
    expect(visible[0].name).toBe('My Project');
  });
});
