// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { store } from '../state/store';

// Mock Tauri APIs
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(() => Promise.resolve()),
}));

vi.mock('../services/terminal-service', () => ({
  terminalService: {
    createTerminal: vi.fn(),
    closeTerminal: vi.fn(),
    writeToTerminal: vi.fn(),
    renameTerminal: vi.fn(() => Promise.resolve()),
  },
}));

vi.mock('../services/workspace-service', () => ({
  workspaceService: {
    reorderTabs: vi.fn(() => Promise.resolve()),
  },
}));

import { TabBar } from './TabBar';

const origRAF = globalThis.requestAnimationFrame;

describe('TabBar rename focus stealing', () => {
  // Bug: when renaming a tab, the terminal auto-focuses and interrupts the rename.
  // Two vectors: (1) store changes trigger re-render that destroys the input,
  // (2) click events on the input bubble to the tab's onclick handler which
  // calls setActiveTerminal, triggering TerminalPane.focusInput().

  let tabBar: TabBar;
  let mountPoint: HTMLElement;

  beforeEach(() => {
    globalThis.requestAnimationFrame = (cb: FrameRequestCallback) => { cb(0); return 0; };

    store.reset();

    store.addWorkspace({
      id: 'ws-1',
      name: 'Test Workspace',
      folderPath: 'C:\\test',
      tabOrder: [],
      shellType: { type: 'windows' },
      worktreeMode: false,
      claudeCodeMode: false,
    });

    store.setActiveWorkspace('ws-1');
    store.addTerminal({ id: 't1', workspaceId: 'ws-1', name: 'Tab 1', processName: 'cmd', order: 0 });
    store.setActiveTerminal('t1');

    tabBar = new TabBar();
    mountPoint = document.createElement('div');
    document.body.appendChild(mountPoint);
    tabBar.mount(mountPoint);
  });

  afterEach(() => {
    document.body.textContent = '';
    globalThis.requestAnimationFrame = origRAF;
    vi.restoreAllMocks();
  });

  function getActiveTitle(): HTMLElement | null {
    return mountPoint.querySelector('.tab.active .tab-title');
  }

  function getRenameInput(): HTMLInputElement | null {
    return mountPoint.querySelector('.tab.active input.tab-title.editing');
  }

  function startRename() {
    const title = getActiveTitle();
    expect(title).not.toBeNull();
    title!.dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
  }

  describe('re-render destroys rename input', () => {
    // Bug: updateTabInPlace() unconditionally replaces any <input> with a <span>
    // on every render cycle. Any store state change during rename kills the input.

    it('should preserve rename input when store state changes during rename', () => {
      startRename();
      const input = getRenameInput();
      expect(input).not.toBeNull();
      input!.value = 'My New Na';

      // Simulate a state change that would occur during normal usage
      // (e.g., terminal output updates process name, another terminal added, etc.)
      store.updateTerminal('t1', { processName: 'node' });

      // The rename input must survive the re-render
      const inputAfter = getRenameInput();
      expect(inputAfter).not.toBeNull();
      expect(inputAfter!.value).toBe('My New Na');
    });

    it('should preserve rename input when a second terminal is added during rename', () => {
      startRename();
      const input = getRenameInput();
      expect(input).not.toBeNull();
      input!.value = 'Renaming';

      // Another terminal being added triggers store notify → render.
      // addTerminal switches activeTerminalId to the new tab, so the
      // renaming tab loses .active — but the input must still survive.
      store.addTerminal({ id: 't2', workspaceId: 'ws-1', name: 'Tab 2', processName: 'cmd', order: 1 });

      // Query without .active since the tab was deactivated by addTerminal
      const inputAfter = mountPoint.querySelector('input.tab-title.editing') as HTMLInputElement | null;
      expect(inputAfter).not.toBeNull();
      expect(inputAfter!.value).toBe('Renaming');
    });

    it('should preserve rename input when setActiveTerminal is called for the same terminal', () => {
      startRename();
      const input = getRenameInput();
      expect(input).not.toBeNull();
      input!.value = 'In Progress';

      // Re-activating the same terminal should not kill the rename
      store.setActiveTerminal('t1');

      const inputAfter = getRenameInput();
      expect(inputAfter).not.toBeNull();
      expect(inputAfter!.value).toBe('In Progress');
    });
  });

  describe('click inside rename input triggers terminal activation', () => {
    // Bug: clicking inside the rename input (to reposition cursor) bubbles up
    // to the parent tab's onclick, which calls store.setActiveTerminal().
    // This triggers TerminalPane.setActive(true) → focusInput(), stealing
    // focus from the rename input.

    it('should not call setActiveTerminal when clicking inside the rename input', () => {
      const spy = vi.spyOn(store, 'setActiveTerminal');
      spy.mockClear();

      startRename();
      const input = getRenameInput()!;

      // Click inside the input (user repositioning cursor)
      input.dispatchEvent(new MouseEvent('click', { bubbles: true }));

      // setActiveTerminal should NOT have been called from this click
      expect(spy).not.toHaveBeenCalled();
    });

    it('should keep focus on rename input after clicking inside it', () => {
      startRename();
      const input = getRenameInput()!;
      input.focus();
      expect(document.activeElement).toBe(input);

      // Click inside the input
      input.dispatchEvent(new MouseEvent('click', { bubbles: true }));

      // Focus must remain on the rename input
      expect(document.activeElement).toBe(input);
      // And the input must still be in the DOM
      expect(getRenameInput()).not.toBeNull();
    });
  });

  describe('typing during rename survives concurrent state changes', () => {
    // Bug: real-world scenario — user types in rename while terminal
    // produces output. The output triggers a store change → re-render
    // which destroys the input mid-keystroke.

    it('should preserve rename input and its value across multiple store updates', () => {
      startRename();
      const input = getRenameInput()!;
      input.focus();

      // Simulate typing character by character with state changes between keystrokes
      input.value = 'M';
      input.dispatchEvent(new Event('input', { bubbles: true }));

      // Terminal output arrives, triggering store update
      store.updateTerminal('t1', { processName: 'python' });

      // Input must survive
      let current = getRenameInput();
      expect(current).not.toBeNull();
      expect(current!.value).toBe('M');

      // User types another character
      current!.value = 'My';
      current!.dispatchEvent(new Event('input', { bubbles: true }));

      // Another state change
      store.updateTerminal('t1', { processName: 'python3' });

      // Input must still survive
      current = getRenameInput();
      expect(current).not.toBeNull();
      expect(current!.value).toBe('My');
    });
  });
});
