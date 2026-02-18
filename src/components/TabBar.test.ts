import { describe, it, expect, beforeEach } from 'vitest';
import { getDisplayName } from './TabBar';
import { Terminal, store } from '../state/store';

function terminal(overrides: Partial<Terminal> = {}): Terminal {
  return {
    id: 't-1',
    workspaceId: 'ws-1',
    name: '',
    processName: '',
    order: 0,
    ...overrides,
  };
}

describe('getDisplayName', () => {
  it('returns user-renamed name even when oscTitle is set', () => {
    // User double-click rename should always win
    expect(getDisplayName(terminal({
      name: 'My Tab',
      oscTitle: 'vim README.md',
      userRenamed: true,
    }))).toBe('My Tab');
  });

  it('returns oscTitle over default name', () => {
    expect(getDisplayName(terminal({
      name: 'Terminal',
      oscTitle: 'claude: fixing bug',
      processName: 'powershell',
    }))).toBe('claude: fixing bug');
  });

  it('returns oscTitle over worktree branch name', () => {
    expect(getDisplayName(terminal({
      name: 'feat/search',
      oscTitle: 'npm test',
    }))).toBe('npm test');
  });

  it('returns name when no oscTitle is set', () => {
    expect(getDisplayName(terminal({
      name: 'feat/search',
      processName: 'powershell',
    }))).toBe('feat/search');
  });

  it('returns processName when name is empty and no oscTitle', () => {
    expect(getDisplayName(terminal({
      name: '',
      processName: 'powershell',
    }))).toBe('powershell');
  });

  it('returns Terminal as last fallback', () => {
    expect(getDisplayName(terminal({
      name: '',
      processName: '',
    }))).toBe('Terminal');
  });

  it('treats undefined oscTitle the same as absent', () => {
    expect(getDisplayName(terminal({
      name: 'Main',
      oscTitle: undefined,
    }))).toBe('Main');
  });

  it('treats empty-string oscTitle as absent (falls through to name)', () => {
    // godly-vt may return an empty title string when title is cleared
    expect(getDisplayName(terminal({
      name: 'Main',
      oscTitle: '',
    }))).toBe('Main');
  });

  it('does not use userRenamed flag when it is false', () => {
    expect(getDisplayName(terminal({
      name: 'Main',
      oscTitle: 'vim',
      userRenamed: false,
    }))).toBe('vim');
  });
});

// Bug: "autopick from claude code to the terminal name" broken.
// When a terminal is created with the default name 'Terminal' and the process
// changes (e.g. powershell -> node when Claude Code starts), the tab should
// show the current process name. Instead, getDisplayName() returned 'Terminal'
// because the default name was truthy and blocked processName.
describe('getDisplayName - process name autopick', () => {
  it('should show processName when name is the generic default "Terminal"', () => {
    // Bug: tab stayed "Terminal" even after process changes to "node" (Claude Code)
    const t = terminal({
      name: 'Terminal',
      processName: 'node',
    });
    expect(getDisplayName(t)).toBe('node');
  });

  it('should show processName "claude" when Claude Code is running', () => {
    const t = terminal({
      name: 'Terminal',
      processName: 'claude',
    });
    expect(getDisplayName(t)).toBe('claude');
  });

  it('should update display when process changes from powershell to node', () => {
    const t = terminal({
      name: 'Terminal',
      processName: 'powershell',
    });
    const updated = { ...t, processName: 'node' };
    expect(getDisplayName(updated)).toBe('node');
  });

  it('should prefer oscTitle over processName when both available', () => {
    const t = terminal({
      name: 'Terminal',
      processName: 'node',
      oscTitle: 'claude: fixing scrollback bug',
    });
    expect(getDisplayName(t)).toBe('claude: fixing scrollback bug');
  });

  it('should still show worktree branch name over processName', () => {
    const t = terminal({
      name: 'feat/search',
      processName: 'node',
    });
    expect(getDisplayName(t)).toBe('feat/search');
  });
});

// Full flow: claudeCodeMode workspace -> terminal created -> process changes
describe('claudeCodeMode tab naming flow', () => {
  beforeEach(() => {
    store.reset();
    store.addWorkspace({
      id: 'ws-claude', name: 'Claude WS', folderPath: 'C:\\Projects',
      tabOrder: [], shellType: { type: 'windows' },
      worktreeMode: false, claudeCodeMode: true,
    });
  });

  it('should reflect process change in tab display name', () => {
    store.addTerminal({
      id: 'cc-term', workspaceId: 'ws-claude',
      name: 'Terminal',
      processName: 'powershell',
      order: 0,
    });

    // Claude Code starts - process-changed event fires
    store.updateTerminal('cc-term', { processName: 'node', oscTitle: undefined });

    const t = store.getState().terminals.find(t => t.id === 'cc-term')!;
    expect(getDisplayName(t)).toBe('node');
  });

  it('should show OSC title from Claude Code when available', () => {
    store.addTerminal({
      id: 'cc-term', workspaceId: 'ws-claude',
      name: 'Terminal',
      processName: 'powershell',
      order: 0,
    });

    store.updateTerminal('cc-term', { oscTitle: 'claude: running tests' });

    const t = store.getState().terminals.find(t => t.id === 'cc-term')!;
    expect(getDisplayName(t)).toBe('claude: running tests');
  });

  it('should fall back to processName when OSC title is cleared', () => {
    store.addTerminal({
      id: 'cc-term', workspaceId: 'ws-claude',
      name: 'Terminal',
      processName: 'node',
      order: 0,
    });

    store.updateTerminal('cc-term', { oscTitle: 'claude: working' });
    store.updateTerminal('cc-term', { processName: 'powershell', oscTitle: undefined });

    const t = store.getState().terminals.find(t => t.id === 'cc-term')!;
    expect(getDisplayName(t)).toBe('powershell');
  });
});
