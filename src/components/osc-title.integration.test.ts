import { describe, it, expect, beforeEach } from 'vitest';
import { Terminal } from '@xterm/headless';
import { store } from '../state/store';
import { getDisplayName } from './TabBar';

// Integration test: OSC escape sequences → xterm.js parser → store → tab display.
// Uses @xterm/headless so we get a real xterm.js parser without needing a DOM.

describe('OSC title integration', () => {
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

  function createWiredTerminal(): Terminal {
    const terminal = new Terminal();
    terminal.onTitleChange((title) => {
      store.updateTerminal(TERMINAL_ID, { oscTitle: title || undefined });
    });
    return terminal;
  }

  function writeOscAndFlush(terminal: Terminal, seq: string): Promise<void> {
    return new Promise((resolve) => {
      terminal.write(seq, resolve);
    });
  }

  function getStoredTerminal() {
    return store.getState().terminals.find(t => t.id === TERMINAL_ID)!;
  }

  it('OSC 0 sequence updates store oscTitle (like Claude Code emitting \\e]0;title\\a)', async () => {
    const terminal = createWiredTerminal();

    // Claude Code emits: ESC ] 0 ; <title> BEL
    await writeOscAndFlush(terminal, '\x1b]0;claude: fixing scrollback bug\x07');

    const t = getStoredTerminal();
    expect(t.oscTitle).toBe('claude: fixing scrollback bug');
    expect(getDisplayName(t)).toBe('claude: fixing scrollback bug');

    terminal.dispose();
  });

  it('OSC 2 sequence also sets the title (alternate form used by some programs)', async () => {
    const terminal = createWiredTerminal();

    // OSC 2 = "Set Window Title" — functionally same as OSC 0 for tab titles
    await writeOscAndFlush(terminal, '\x1b]2;vim README.md\x07');

    expect(getStoredTerminal().oscTitle).toBe('vim README.md');

    terminal.dispose();
  });

  it('successive OSC sequences update the title (Claude Code changes task status)', async () => {
    const terminal = createWiredTerminal();

    await writeOscAndFlush(terminal, '\x1b]0;claude: reading files\x07');
    expect(getDisplayName(getStoredTerminal())).toBe('claude: reading files');

    await writeOscAndFlush(terminal, '\x1b]0;claude: running tests\x07');
    expect(getDisplayName(getStoredTerminal())).toBe('claude: running tests');

    terminal.dispose();
  });

  it('OSC title embedded in normal output is still parsed', async () => {
    const terminal = createWiredTerminal();

    // Real terminal output: regular text mixed with an OSC sequence
    await writeOscAndFlush(terminal, 'Building project...\r\n\x1b]0;npm run build\x07Done.\r\n');

    expect(getStoredTerminal().oscTitle).toBe('npm run build');

    terminal.dispose();
  });

  it('ST terminator (ESC \\) works as well as BEL (\\x07)', async () => {
    const terminal = createWiredTerminal();

    // Some programs use ESC \ (ST) instead of BEL to terminate OSC
    await writeOscAndFlush(terminal, '\x1b]0;htop\x1b\\');

    expect(getStoredTerminal().oscTitle).toBe('htop');

    terminal.dispose();
  });

  it('empty OSC title clears oscTitle back to undefined', async () => {
    const terminal = createWiredTerminal();

    await writeOscAndFlush(terminal, '\x1b]0;something\x07');
    expect(getStoredTerminal().oscTitle).toBe('something');

    // Program clears its title (e.g., on exit) — xterm.js fires onTitleChange('')
    await writeOscAndFlush(terminal, '\x1b]0;\x07');
    expect(getStoredTerminal().oscTitle).toBeUndefined();

    // Display falls back to name since oscTitle is cleared
    expect(getDisplayName(getStoredTerminal())).toBe('Terminal');

    terminal.dispose();
  });

  it('userRenamed tab is not affected by OSC titles', async () => {
    // Simulate user double-click rename
    store.updateTerminal(TERMINAL_ID, { name: 'My Build Tab', userRenamed: true });

    const terminal = createWiredTerminal();

    await writeOscAndFlush(terminal, '\x1b]0;some program title\x07');

    const t = getStoredTerminal();
    // OSC title IS stored (in case user un-renames later)
    expect(t.oscTitle).toBe('some program title');
    // But getDisplayName returns the user-chosen name
    expect(getDisplayName(t)).toBe('My Build Tab');

    terminal.dispose();
  });
});
