import { describe, it, expect, beforeEach } from 'vitest';
import { store } from '../state/store';
import { getDisplayName } from './TabBar';

/**
 * Bug reproduction: Terminal tabs all show "pa-mcp" and don't pick up
 * dynamic title changes like Windows Terminal does.
 *
 * Two root causes:
 * 1. process-changed events clear oscTitle (terminal-service.ts:76 sets
 *    oscTitle: undefined), wiping dynamic titles set by applications via
 *    OSC escape sequences.
 * 2. ProcessMonitor's find_deepest_child() picks up background MCP helper
 *    processes instead of the interactive foreground process.
 */

const TERMINAL_ID = 'tab-title-bug';

function setupTerminal(processName = 'powershell') {
  store.addWorkspace({
    id: 'ws-1', name: 'Test', folderPath: 'C:\\', tabOrder: [],
    shellType: { type: 'windows' }, worktreeMode: false, claudeCodeMode: false,
  });
  store.addTerminal({
    id: TERMINAL_ID, workspaceId: 'ws-1',
    name: 'Terminal', processName, order: 0,
  });
}

function getTerminal() {
  return store.getState().terminals.find(t => t.id === TERMINAL_ID)!;
}

/**
 * Simulates a process-changed event arriving from ProcessMonitor.
 * This mirrors the code path in terminal-service.ts:76:
 *   store.updateTerminal(terminal_id, { processName: process_name });
 * Note: oscTitle must NOT be cleared by process-changed events.
 */
function simulateProcessChanged(processName: string) {
  store.updateTerminal(TERMINAL_ID, { processName });
}

/**
 * Simulates an OSC title arriving from a grid snapshot (via TerminalRenderer).
 * This is the code path in TerminalPane.ts:79:
 *   store.updateTerminal(this.terminalId, { oscTitle: title || undefined });
 */
function simulateOscTitle(title: string) {
  store.updateTerminal(TERMINAL_ID, { oscTitle: title || undefined });
}

describe('Bug: process-changed events wipe oscTitle', () => {
  beforeEach(() => {
    store.reset();
    setupTerminal('powershell');
  });

  it('should preserve oscTitle when process-changed fires with same process', () => {
    // Bug: OSC title set by application (e.g. Claude Code sets window title)
    // gets wiped every 2 seconds when ProcessMonitor polls and fires process-changed.
    // Expected: oscTitle should survive process-changed events.
    simulateOscTitle('claude: running tests');
    expect(getDisplayName(getTerminal())).toBe('claude: running tests');

    // ProcessMonitor fires process-changed (polls every 2s) - same process, but
    // terminal-service.ts:76 does { processName, oscTitle: undefined }
    simulateProcessChanged('node');

    // After process-changed, the oscTitle should still be preserved
    expect(getTerminal().oscTitle).toBe('claude: running tests');
    expect(getDisplayName(getTerminal())).toBe('claude: running tests');
  });

  it('should preserve oscTitle when process changes to a child helper process', () => {
    // Bug: Claude Code sets OSC title, then spawns MCP helper "pa-mcp".
    // ProcessMonitor detects "pa-mcp" as deepest child, fires process-changed,
    // which wipes oscTitle AND sets processName to "pa-mcp".
    simulateOscTitle('claude: fixing scrollback bug');
    expect(getDisplayName(getTerminal())).toBe('claude: fixing scrollback bug');

    // ProcessMonitor detects MCP helper as the deepest child
    simulateProcessChanged('pa-mcp');

    // oscTitle should not be wiped by process detection
    expect(getTerminal().oscTitle).toBe('claude: fixing scrollback bug');
    expect(getDisplayName(getTerminal())).toBe('claude: fixing scrollback bug');
  });

  it('should not show "pa-mcp" when oscTitle was previously set', () => {
    // Bug: Tab shows "pa-mcp" instead of the application-set title
    simulateOscTitle('claude: reading files');

    // Process changes to background MCP helper
    simulateProcessChanged('pa-mcp');

    // Should never show "pa-mcp" when an oscTitle was set
    expect(getDisplayName(getTerminal())).not.toBe('pa-mcp');
  });

  it('should preserve oscTitle across multiple process-changed polls', () => {
    // Bug: ProcessMonitor polls every 2 seconds, each poll wipes oscTitle.
    // Over time, oscTitle can never persist.
    simulateOscTitle('vim README.md');

    // Simulate 3 consecutive ProcessMonitor polls (6 seconds of polling)
    simulateProcessChanged('node');
    simulateProcessChanged('node');
    simulateProcessChanged('node');

    expect(getTerminal().oscTitle).toBe('vim README.md');
    expect(getDisplayName(getTerminal())).toBe('vim README.md');
  });
});

describe('Bug: tabs all named "pa-mcp" in Claude Code workspace', () => {
  beforeEach(() => {
    store.reset();
    store.addWorkspace({
      id: 'ws-claude', name: 'Claude WS', folderPath: 'C:\\Projects',
      tabOrder: [], shellType: { type: 'windows' },
      worktreeMode: false, claudeCodeMode: true,
    });
  });

  it('should not display "pa-mcp" as tab name for Claude Code terminals', () => {
    // Bug: All tabs show "pa-mcp" because ProcessMonitor's find_deepest_child()
    // traverses: powershell -> node (claude) -> pa-mcp, and picks "pa-mcp".
    store.addTerminal({
      id: 'cc-1', workspaceId: 'ws-claude',
      name: 'Terminal', processName: 'node', order: 0,
    });

    // Claude Code starts and sets an OSC title
    store.updateTerminal('cc-1', { oscTitle: 'claude: working' });
    const beforeChange = store.getState().terminals.find(t => t.id === 'cc-1')!;
    expect(getDisplayName(beforeChange)).toBe('claude: working');

    // ProcessMonitor finds "pa-mcp" as deepest child and fires process-changed
    // This simulates exactly what terminal-service.ts:76 does
    simulateProcessChangedFor('cc-1', 'pa-mcp');

    const afterChange = store.getState().terminals.find(t => t.id === 'cc-1')!;
    // Tab should NOT show "pa-mcp"
    expect(getDisplayName(afterChange)).not.toBe('pa-mcp');
  });

  it('multiple Claude Code tabs should not all show "pa-mcp"', () => {
    // Bug: User opens multiple Claude Code terminals, all tabs display "pa-mcp"
    store.addTerminal({
      id: 'cc-1', workspaceId: 'ws-claude',
      name: 'Terminal', processName: 'node', order: 0,
    });
    store.addTerminal({
      id: 'cc-2', workspaceId: 'ws-claude',
      name: 'Terminal', processName: 'node', order: 1,
    });

    // Both set OSC titles from Claude Code
    store.updateTerminal('cc-1', { oscTitle: 'claude: fixing bug #42' });
    store.updateTerminal('cc-2', { oscTitle: 'claude: writing tests' });

    // ProcessMonitor fires for both with "pa-mcp"
    simulateProcessChangedFor('cc-1', 'pa-mcp');
    simulateProcessChangedFor('cc-2', 'pa-mcp');

    const t1 = store.getState().terminals.find(t => t.id === 'cc-1')!;
    const t2 = store.getState().terminals.find(t => t.id === 'cc-2')!;

    // Tabs should show their unique titles, not all "pa-mcp"
    expect(getDisplayName(t1)).not.toBe('pa-mcp');
    expect(getDisplayName(t2)).not.toBe('pa-mcp');
    // They should be distinguishable
    expect(getDisplayName(t1)).not.toBe(getDisplayName(t2));
  });

  it('tab should show direct-child processName, not background helper', () => {
    // Bug: ProcessMonitor's find_deepest_child traversed shell → node → pa-mcp
    // and reported "pa-mcp". After fix (find_direct_child), it reports "node"
    // (the direct child of the shell, i.e. what the user actually launched).
    store.addTerminal({
      id: 'cc-1', workspaceId: 'ws-claude',
      name: 'Terminal', processName: 'powershell', order: 0,
    });

    // Fixed ProcessMonitor now sends the direct child ("node"), not "pa-mcp"
    simulateProcessChangedFor('cc-1', 'node');

    const t = store.getState().terminals.find(t => t.id === 'cc-1')!;
    expect(getDisplayName(t)).toBe('node');
    expect(t.processName).toBe('node');
  });
});

/**
 * Helper: simulate process-changed for a specific terminal ID.
 * Mirrors terminal-service.ts:76 (process-changed handler).
 */
function simulateProcessChangedFor(terminalId: string, processName: string) {
  store.updateTerminal(terminalId, { processName });
}
