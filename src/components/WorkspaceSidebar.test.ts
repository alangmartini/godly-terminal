// @vitest-environment jsdom

/**
 * Bug #612: Folder picker doesn't open when creating a new workspace.
 *
 * handleAddWorkspace() calls open() from @tauri-apps/plugin-dialog with
 * { directory: true } but has NO try-catch. When the native folder picker
 * fails (permission denied, COM error, plugin misconfiguration), the error
 * silently propagates as an unhandled promise rejection and the user gets
 * zero feedback.
 */

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

// ── Mocks (must be before imports) ──────────────────────────────────

const mockOpen = vi.fn();
vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: (...args: unknown[]) => mockOpen(...args),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
  openPath: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('./WorktreePanel', () => ({
  WorktreePanel: class {
    mount() { /* no-op */ }
  },
}));

vi.mock('../state/drag-state', () => ({
  startDrag: vi.fn(),
  getDrag: vi.fn(() => null),
  endDrag: vi.fn(),
  createGhost: vi.fn(),
  moveGhost: vi.fn(),
  onDragMove: vi.fn(),
  onDragDrop: vi.fn(),
  notifyMove: vi.fn(),
  notifyDrop: vi.fn(),
}));

const mockIsWslAvailable = vi.fn().mockResolvedValue(false);
const mockGetWslDistributions = vi.fn().mockResolvedValue([]);
const mockCreateWorkspace = vi.fn().mockResolvedValue('ws-new');

vi.mock('../services/workspace-service', () => ({
  workspaceService: {
    isWslAvailable: (...args: unknown[]) => mockIsWslAvailable(...args),
    getWslDistributions: (...args: unknown[]) => mockGetWslDistributions(...args),
    createWorkspace: (...args: unknown[]) => mockCreateWorkspace(...args),
    isGitRepo: vi.fn().mockResolvedValue(false),
    toggleWorktreeMode: vi.fn(),
    cycleAiToolMode: vi.fn(),
    setAiToolMode: vi.fn(),
  },
}));

vi.mock('../state/notification-store', () => ({
  notificationStore: {
    subscribe: vi.fn(),
    isWorkspaceNotificationEnabled: vi.fn(() => true),
    workspaceHasBadge: vi.fn(() => false),
    setWorkspaceOverride: vi.fn(),
  },
}));

// ── Imports (after mocks) ───────────────────────────────────────────

import { store } from '../state/store';
import { WorkspaceSidebar } from './WorkspaceSidebar';

// ── Helpers ─────────────────────────────────────────────────────────

/** Flush microtask queue and pending timers */
async function tick(ms = 10) {
  await new Promise(r => setTimeout(r, ms));
}

/** Click Continue in the shell type dialog */
function clickShellTypeContinue(): HTMLButtonElement | null {
  const btns = document.querySelectorAll<HTMLButtonElement>('.dialog-btn-primary');
  // The shell type dialog's Continue button is the one in the overlay
  for (const btn of btns) {
    if (btn.textContent === 'Continue') {
      btn.click();
      return btn;
    }
  }
  return null;
}

// ── Tests ───────────────────────────────────────────────────────────

describe('WorkspaceSidebar — New Workspace flow (Bug #612)', () => {
  let sidebar: WorkspaceSidebar;
  let container: HTMLElement;

  beforeEach(() => {
    vi.clearAllMocks();
    document.body.innerHTML = '';

    // Re-set mock return values after clearAllMocks
    mockIsWslAvailable.mockResolvedValue(false);
    mockGetWslDistributions.mockResolvedValue([]);
    mockCreateWorkspace.mockResolvedValue('ws-new');

    // Reset store
    store.setState({
      workspaces: [],
      terminals: [],
      activeWorkspaceId: null,
      activeTerminalId: null,
    });

    container = document.createElement('div');
    document.body.appendChild(container);

    sidebar = new WorkspaceSidebar();
    sidebar.mount(container);
  });

  afterEach(() => {
    document.body.innerHTML = '';
    vi.restoreAllMocks();
  });

  it('should call the folder picker with directory:true after shell type selection', async () => {
    // Bug #612: folder picker should open after shell type dialog
    mockOpen.mockResolvedValueOnce('C:\\Projects\\test');

    // Trigger the flow
    const promise = (sidebar as any).handleAddWorkspace();
    await tick();

    // Shell type dialog should be visible
    const overlay = document.querySelector('.dialog-overlay');
    expect(overlay).not.toBeNull();

    // Click Continue
    clickShellTypeContinue();
    await tick();

    await promise;

    expect(mockOpen).toHaveBeenCalledWith({
      directory: true,
      multiple: false,
      title: 'Select workspace folder',
    });
  });

  it('should create workspace with folder name from the selected path', async () => {
    // Bug #612: when folder is selected, workspace should be created
    mockOpen.mockResolvedValueOnce('C:\\Users\\alan\\Projects\\my-app');

    const promise = (sidebar as any).handleAddWorkspace();
    await tick();
    clickShellTypeContinue();
    await tick();
    await promise;

    expect(mockCreateWorkspace).toHaveBeenCalledWith(
      'my-app',
      'C:\\Users\\alan\\Projects\\my-app',
      { type: 'windows' },
    );
  });

  it('should remove the shell type dialog overlay before opening folder picker', async () => {
    // Bug #612: if the dialog overlay stays on screen, it could block
    // the native folder picker from receiving input
    let overlayPresentWhenOpenCalled = true;

    mockOpen.mockImplementation(async () => {
      // Check if any dialog overlay remains when open() is called
      overlayPresentWhenOpenCalled = document.querySelector('.dialog-overlay') !== null;
      return 'C:\\test';
    });

    const promise = (sidebar as any).handleAddWorkspace();
    await tick();
    clickShellTypeContinue();
    await tick();
    await promise;

    expect(overlayPresentWhenOpenCalled).toBe(false);
  });

  it('should handle folder picker errors without unhandled rejection', async () => {
    // Bug #612: open() has no try-catch — when the native folder picker
    // fails (e.g., Tauri permission denied, COM error), the error becomes
    // an unhandled promise rejection and the user gets zero feedback.
    //
    // Expected: handleAddWorkspace() catches the error gracefully.
    // Actual: the promise rejects → unhandled rejection → silent failure.

    mockOpen.mockRejectedValueOnce(new Error('Failed to open native folder picker'));

    const promise = (sidebar as any).handleAddWorkspace();
    await tick();
    clickShellTypeContinue();
    await tick();

    // The promise from handleAddWorkspace() should resolve (not reject),
    // because the error should be caught internally.
    // This FAILS on current code because there's no try-catch around open().
    await expect(promise).resolves.not.toThrow();
  });

  it('should not leave dangling overlays when folder picker throws', async () => {
    // Bug #612: if open() throws, verify no stale UI elements remain
    mockOpen.mockRejectedValueOnce(new Error('Dialog permission denied'));

    const promise = (sidebar as any).handleAddWorkspace();
    await tick();
    clickShellTypeContinue();

    // Catch the rejection to prevent test framework noise
    await promise.catch(() => {});
    await tick();

    const overlays = document.querySelectorAll('.dialog-overlay');
    expect(overlays.length).toBe(0);
  });

  it('should not call createWorkspace when folder picker is cancelled', async () => {
    // User cancels the folder picker → null is returned → no workspace created
    mockOpen.mockResolvedValueOnce(null);

    const promise = (sidebar as any).handleAddWorkspace();
    await tick();
    clickShellTypeContinue();
    await tick();
    await promise;

    expect(mockCreateWorkspace).not.toHaveBeenCalled();
  });

  it('should not call open() when shell type dialog is cancelled', async () => {
    // User cancels the shell type dialog → flow aborts before folder picker
    const promise = (sidebar as any).handleAddWorkspace();
    await tick();

    // Click Cancel instead of Continue
    const cancelBtn = document.querySelector('.dialog-btn-secondary');
    expect(cancelBtn).not.toBeNull();
    (cancelBtn as HTMLButtonElement).click();
    await tick();
    await promise;

    expect(mockOpen).not.toHaveBeenCalled();
  });
});
