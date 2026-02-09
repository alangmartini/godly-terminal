import { describe, it, expect } from 'vitest';
import { getDisplayName } from './TabBar';
import { Terminal } from '../state/store';

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
    // xterm.js may fire onTitleChange('') when title is cleared
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
