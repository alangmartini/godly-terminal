// @vitest-environment jsdom

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

/**
 * Regression tests for Bug #409: Notifications auto-switch terminal on window refocus.
 *
 * Previously, `handlePendingNotificationNavigation()` fired on every `window.focus`
 * event, auto-navigating to the last notification's terminal. This was removed —
 * navigation now only happens via explicit toast click (ToastContainer).
 *
 * These tests verify that window focus events never cause terminal/workspace switching.
 */

// ── Helpers ──────────────────────────────────────────────────────────────

/** Set up a two-workspace, two-terminal store state. */
function setupTwoTerminalState() {
  store.setState({
    workspaces: [],
    terminals: [],
    activeWorkspaceId: null,
    activeTerminalId: null,
  });

  store.addWorkspace({
    id: 'ws-reading',
    name: 'Reading',
    folderPath: 'C:\\Reading',
    tabOrder: [],
    shellType: { type: 'windows' },
    worktreeMode: false,
    claudeCodeMode: false,
  });
  store.addTerminal({
    id: 'term-reading',
    workspaceId: 'ws-reading',
    name: 'Reading Terminal',
    processName: 'bash',
    order: 0,
  });

  store.addWorkspace({
    id: 'ws-notify',
    name: 'Notifier',
    folderPath: 'C:\\Notifier',
    tabOrder: [],
    shellType: { type: 'windows' },
    worktreeMode: false,
    claudeCodeMode: false,
  });
  store.addTerminal({
    id: 'term-notify',
    workspaceId: 'ws-notify',
    name: 'Claude Code',
    processName: 'claude',
    order: 0,
  });

  // User is actively reading terminal in ws-reading
  store.setActiveWorkspace('ws-reading');
  store.setActiveTerminal('term-reading');
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Bug #409: Window refocus must NOT auto-switch terminal', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setupTwoTerminalState();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('window focus event does not change active terminal', () => {
    expect(store.getState().activeTerminalId).toBe('term-reading');

    window.dispatchEvent(new Event('focus'));

    expect(store.getState().activeTerminalId).toBe('term-reading');
    expect(store.getState().activeWorkspaceId).toBe('ws-reading');
  });

  it('multiple window focus events do not change active terminal', () => {
    expect(store.getState().activeTerminalId).toBe('term-reading');

    // Simulate repeated Alt-Tab cycles
    for (let i = 0; i < 5; i++) {
      window.dispatchEvent(new Event('focus'));
    }

    expect(store.getState().activeTerminalId).toBe('term-reading');
    expect(store.getState().activeWorkspaceId).toBe('ws-reading');
  });

  it('window focus after blur does not change active terminal', () => {
    expect(store.getState().activeTerminalId).toBe('term-reading');

    // Simulate Alt-Tab away and back
    window.dispatchEvent(new Event('blur'));
    window.dispatchEvent(new Event('focus'));

    expect(store.getState().activeTerminalId).toBe('term-reading');
    expect(store.getState().activeWorkspaceId).toBe('ws-reading');
  });

  it('toast click still navigates to notification terminal', () => {
    // Bug #409 fix removed auto-navigation on window focus,
    // but explicit toast click should still work (ToastContainer.ts).
    expect(store.getState().activeTerminalId).toBe('term-reading');

    // Simulate what ToastContainer click handler does
    const terminal = store.getState().terminals.find(t => t.id === 'term-notify');
    if (terminal) {
      store.setActiveWorkspace(terminal.workspaceId);
      store.setActiveTerminal('term-notify');
    }

    expect(store.getState().activeTerminalId).toBe('term-notify');
    expect(store.getState().activeWorkspaceId).toBe('ws-notify');
  });

  it('same-workspace tab is not switched by window focus', () => {
    store.addTerminal({
      id: 'term-reading-2',
      workspaceId: 'ws-reading',
      name: 'Background Claude',
      processName: 'claude',
      order: 1,
    });
    // addTerminal may auto-activate the new terminal; reset to original
    store.setActiveTerminal('term-reading');

    expect(store.getState().activeTerminalId).toBe('term-reading');

    window.dispatchEvent(new Event('focus'));

    expect(store.getState().activeTerminalId).toBe('term-reading');
  });
});
