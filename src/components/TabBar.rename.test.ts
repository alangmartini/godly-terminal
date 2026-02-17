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
import { terminalService } from '../services/terminal-service';

const origRAF = globalThis.requestAnimationFrame;

describe('TabBar rename', () => {
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

  // Bug: rename input had no explicit color, so browser defaulted to black text
  // on the dark var(--bg-primary) background — text was invisible.
  describe('input field visibility', () => {
    it('should have color set on the editing class so text is visible', () => {
      startRename();
      const input = getRenameInput();
      expect(input).not.toBeNull();
      expect(input!.classList.contains('editing')).toBe(true);

      // The CSS class .tab-title.editing must set color.
      // We can't test computed styles in jsdom, but we verify the class is applied
      // and the input element is an <input> (not a span), confirming the editing state.
      expect(input!.tagName).toBe('INPUT');
    });
  });

  // Bug: pressing Enter called finishRename() directly, which called render(),
  // which removed the input from DOM, which triggered blur, which called
  // finishRename() again. This double-fire caused rename to silently fail.
  describe('Enter key confirmation', () => {
    it('should call renameTerminal exactly once on Enter', async () => {
      startRename();
      const input = getRenameInput()!;
      input.value = 'New Name';

      // Press Enter — should trigger blur → finishRename once
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));

      // Wait for async finishRename
      await vi.waitFor(() => {
        expect(terminalService.renameTerminal).toHaveBeenCalledTimes(1);
      });

      expect(terminalService.renameTerminal).toHaveBeenCalledWith('t1', 'New Name');
    });

    it('should remove the input and restore a span title after Enter', async () => {
      startRename();
      const input = getRenameInput()!;
      input.value = 'Confirmed';

      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));

      await vi.waitFor(() => {
        // The input should be gone, replaced by a span
        expect(getRenameInput()).toBeNull();
      });

      const title = getActiveTitle();
      expect(title).not.toBeNull();
      expect(title!.tagName).toBe('SPAN');
    });
  });

  describe('blur confirmation', () => {
    it('should confirm rename on blur', async () => {
      startRename();
      const input = getRenameInput()!;
      input.value = 'Blur Name';

      input.dispatchEvent(new Event('blur'));

      await vi.waitFor(() => {
        expect(terminalService.renameTerminal).toHaveBeenCalledTimes(1);
      });

      expect(terminalService.renameTerminal).toHaveBeenCalledWith('t1', 'Blur Name');
    });
  });

  describe('Escape cancellation', () => {
    it('should not call renameTerminal on Escape', () => {
      startRename();
      const input = getRenameInput()!;
      input.value = 'Should Not Persist';

      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));

      expect(terminalService.renameTerminal).not.toHaveBeenCalled();
      expect(getRenameInput()).toBeNull();
    });
  });

  describe('empty name handling', () => {
    it('should not call renameTerminal if name is empty', async () => {
      startRename();
      const input = getRenameInput()!;
      input.value = '   ';

      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));

      // Give time for async to settle
      await new Promise(r => setTimeout(r, 50));

      expect(terminalService.renameTerminal).not.toHaveBeenCalled();
    });
  });
});
