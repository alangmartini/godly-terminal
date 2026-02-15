import { describe, it, expect, beforeEach } from 'vitest';
import { store } from '../state/store';
import { getDisplayName } from './TabBar';

// Integration test: OSC title from grid snapshot -> store -> tab display.
//
// In the godly-vt pipeline, the daemon parses OSC escape sequences and the
// parsed title is included in the RichGridData snapshot's `title` field.
// When TerminalPane receives a snapshot, it calls:
//   store.updateTerminal(id, { oscTitle: title || undefined })
//
// This test verifies the store + tab display behavior, which is the same
// regardless of whether the title came from xterm.js or godly-vt.

describe('OSC title integration (godly-vt pipeline)', () => {
  const TERMINAL_ID = 'osc-test-terminal';

  beforeEach(() => {
    store.reset();
    store.addWorkspace({
      id: 'ws-1', name: 'Test', folderPath: 'C:\\', tabOrder: [],
      shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
    });
    store.addTerminal({
      id: TERMINAL_ID, workspaceId: 'ws-1',
      name: 'Terminal', processName: 'powershell', order: 0,
    });
  });

  /**
   * Simulate what TerminalPane does when it receives a grid snapshot with a title.
   * The renderer fires onTitleChange, which calls:
   *   store.updateTerminal(id, { oscTitle: title || undefined })
   */
  function simulateTitleFromSnapshot(title: string) {
    store.updateTerminal(TERMINAL_ID, { oscTitle: title || undefined });
  }

  function getStoredTerminal() {
    return store.getState().terminals.find(t => t.id === TERMINAL_ID)!;
  }

  it('grid snapshot title updates store oscTitle (like Claude Code emitting OSC 0)', () => {
    // Daemon parses: ESC ] 0 ; <title> BEL and puts it in snapshot.title
    simulateTitleFromSnapshot('claude: fixing scrollback bug');

    const t = getStoredTerminal();
    expect(t.oscTitle).toBe('claude: fixing scrollback bug');
    expect(getDisplayName(t)).toBe('claude: fixing scrollback bug');
  });

  it('successive snapshots update the title (Claude Code changes task status)', () => {
    simulateTitleFromSnapshot('claude: reading files');
    expect(getDisplayName(getStoredTerminal())).toBe('claude: reading files');

    simulateTitleFromSnapshot('claude: running tests');
    expect(getDisplayName(getStoredTerminal())).toBe('claude: running tests');
  });

  it('empty title clears oscTitle back to undefined', () => {
    simulateTitleFromSnapshot('something');
    expect(getStoredTerminal().oscTitle).toBe('something');

    // Program clears its title (e.g., on exit) — daemon returns empty string
    simulateTitleFromSnapshot('');
    expect(getStoredTerminal().oscTitle).toBeUndefined();

    // Display falls back to name since oscTitle is cleared
    expect(getDisplayName(getStoredTerminal())).toBe('Terminal');
  });

  it('userRenamed tab is not affected by OSC titles', () => {
    // Simulate user double-click rename
    store.updateTerminal(TERMINAL_ID, { name: 'My Build Tab', userRenamed: true });

    simulateTitleFromSnapshot('some program title');

    const t = getStoredTerminal();
    // OSC title IS stored (in case user un-renames later)
    expect(t.oscTitle).toBe('some program title');
    // But getDisplayName returns the user-chosen name
    expect(getDisplayName(t)).toBe('My Build Tab');
  });

  it('title persists across multiple renders with the same title', () => {
    simulateTitleFromSnapshot('npm run build');
    simulateTitleFromSnapshot('npm run build');
    simulateTitleFromSnapshot('npm run build');

    expect(getStoredTerminal().oscTitle).toBe('npm run build');
  });

  it('long OSC title is stored as-is (no truncation at store level)', () => {
    const longTitle = 'a'.repeat(500);
    simulateTitleFromSnapshot(longTitle);

    expect(getStoredTerminal().oscTitle).toBe(longTitle);
  });

  it('title with special characters is preserved', () => {
    simulateTitleFromSnapshot('vim ~/projects/godly-terminal/src/main.ts [+]');
    expect(getStoredTerminal().oscTitle).toBe('vim ~/projects/godly-terminal/src/main.ts [+]');
  });
});
