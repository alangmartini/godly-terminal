// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Tests for @ file/folder autocomplete in the Quick Claude dialog.
 *
 * Typing @ in the textarea triggers a file browser dropdown that lists
 * directory contents from the selected workspace root. Directories appear
 * first with trailing /. Selecting a dir navigates into it; selecting a
 * file inserts the full path and closes the dropdown.
 */

// ── Mock data ────────────────────────────────────────────────────────────

const ROOT_ENTRIES = [
  { name: 'src', is_dir: true },
  { name: 'docs', is_dir: true },
  { name: 'App.ts', is_dir: false },
  { name: 'package.json', is_dir: false },
];

const SRC_ENTRIES = [
  { name: 'components', is_dir: true },
  { name: 'index.ts', is_dir: false },
  { name: 'utils.ts', is_dir: false },
];

const WORKSPACES = [
  { id: 'ws-a', name: 'Project Alpha', folderPath: '/projects/alpha' },
  { id: 'ws-b', name: 'Project Beta', folderPath: '/projects/beta' },
];

const WORKSPACE_A_SKILLS = [
  { name: 'deploy', description: 'Deploy', usage: '/deploy', source: 'project' },
];

// ── Setup ────────────────────────────────────────────────────────────────

let mockInvoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  document.body.innerHTML = '';
  localStorage.clear();

  mockInvoke = vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    if (cmd === 'list_directory') {
      const path = args?.path as string;
      if (path === '/projects/alpha') return ROOT_ENTRIES;
      if (path === '/projects/alpha/src') return SRC_ENTRIES;
      if (path === '/projects/beta') return [
        { name: 'lib', is_dir: true },
        { name: 'main.rs', is_dir: false },
      ];
      return [];
    }
    if (cmd === 'list_skills') return WORKSPACE_A_SKILLS;
    return null;
  });

  vi.doMock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.doUnmock('@tauri-apps/api/core');
});

// ── Helpers ──────────────────────────────────────────────────────────────

async function openDialog() {
  const { showQuickClaudeDialog } = await import('./dialogs');
  const resultPromise = showQuickClaudeDialog({
    workspaces: WORKSPACES,
    activeWorkspaceId: 'ws-a',
  });

  await vi.dynamicImportSettled?.() ?? new Promise(r => setTimeout(r, 0));

  const overlay = document.querySelector('.dialog-overlay') as HTMLDivElement;
  const promptArea = overlay.querySelector('textarea.dialog-input') as HTMLTextAreaElement;
  const workspaceSelect = overlay.querySelector('select') as HTMLSelectElement;
  const fileDropdown = overlay.querySelector('.file-dropdown') as HTMLDivElement;

  return { resultPromise, overlay, promptArea, workspaceSelect, fileDropdown };
}

function getFileNames(dropdown: HTMLDivElement): string[] {
  return Array.from(dropdown.querySelectorAll('.file-item-name'))
    .map(el => el.textContent ?? '');
}

async function typeAt(promptArea: HTMLTextAreaElement, value = '@') {
  promptArea.value = value;
  promptArea.selectionStart = value.length;
  promptArea.selectionEnd = value.length;
  promptArea.dispatchEvent(new Event('input', { bubbles: true }));
  await new Promise(r => setTimeout(r, 50));
}

function pressKey(el: HTMLElement, key: string, opts: Record<string, boolean> = {}) {
  el.dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true, ...opts }));
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Quick Claude @ file autocomplete', () => {
  it('shows root listing when typing @, dirs first with trailing /', async () => {
    const { promptArea, fileDropdown } = await openDialog();

    await typeAt(promptArea);

    expect(fileDropdown.style.display).not.toBe('none');
    const names = getFileNames(fileDropdown);
    // Dirs first (with /), then files
    expect(names[0]).toBe('src/');
    expect(names[1]).toBe('docs/');
    expect(names[2]).toBe('App.ts');
    expect(names[3]).toBe('package.json');
  });

  it('selecting a directory navigates into it', async () => {
    const { promptArea, fileDropdown } = await openDialog();

    await typeAt(promptArea);

    // Click the "src/" item
    const srcItem = fileDropdown.querySelector('.file-item') as HTMLDivElement;
    srcItem.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    await new Promise(r => setTimeout(r, 50));

    // Textarea should now have @src/
    expect(promptArea.value).toBe('@src/');
    // Dropdown should refresh to show src/ contents
    const names = getFileNames(fileDropdown);
    expect(names).toContain('components/');
    expect(names).toContain('index.ts');
  });

  it('selecting a file inserts full path + space and closes dropdown', async () => {
    const { promptArea, fileDropdown } = await openDialog();

    await typeAt(promptArea);

    // Find the App.ts item (3rd item, 0-indexed: index 2)
    const items = fileDropdown.querySelectorAll('.file-item');
    const appItem = items[2] as HTMLDivElement;
    appItem.dispatchEvent(new MouseEvent('mousedown', { bubbles: true }));
    await new Promise(r => setTimeout(r, 50));

    expect(promptArea.value).toBe('@App.ts ');
    expect(fileDropdown.style.display).toBe('none');
  });

  it('filters entries by substring match', async () => {
    const { promptArea, fileDropdown } = await openDialog();

    await typeAt(promptArea, '@App');

    const names = getFileNames(fileDropdown);
    expect(names).toContain('App.ts');
    expect(names).not.toContain('package.json');
    expect(names).not.toContain('src/');
  });

  it('Escape closes file dropdown', async () => {
    const { promptArea, fileDropdown } = await openDialog();

    await typeAt(promptArea);
    expect(fileDropdown.style.display).not.toBe('none');

    pressKey(promptArea, 'Escape');
    expect(fileDropdown.style.display).toBe('none');
  });

  it('arrow keys navigate and Tab selects', async () => {
    const { promptArea, fileDropdown } = await openDialog();

    await typeAt(promptArea);

    // First item (src/) should be highlighted by default
    let activeItems = fileDropdown.querySelectorAll('.file-item-active');
    expect(activeItems.length).toBe(1);
    expect(activeItems[0].querySelector('.file-item-name')?.textContent).toBe('src/');

    // ArrowDown to move to second item (docs/)
    pressKey(promptArea, 'ArrowDown');
    activeItems = fileDropdown.querySelectorAll('.file-item-active');
    expect(activeItems[0].querySelector('.file-item-name')?.textContent).toBe('docs/');

    // ArrowUp back to first
    pressKey(promptArea, 'ArrowUp');
    activeItems = fileDropdown.querySelectorAll('.file-item-active');
    expect(activeItems[0].querySelector('.file-item-name')?.textContent).toBe('src/');

    // Tab to select the current item (src/) — should navigate into it
    pressKey(promptArea, 'Tab');
    await new Promise(r => setTimeout(r, 50));
    expect(promptArea.value).toBe('@src/');
  });

  it('workspace change clears cache and re-fetches', async () => {
    const { promptArea, workspaceSelect, fileDropdown } = await openDialog();

    await typeAt(promptArea);
    expect(mockInvoke).toHaveBeenCalledWith('list_directory', { path: '/projects/alpha' });

    // Change workspace
    workspaceSelect.value = 'ws-b';
    workspaceSelect.dispatchEvent(new Event('change', { bubbles: true }));
    await new Promise(r => setTimeout(r, 50));

    // Should have fetched from the new workspace
    expect(mockInvoke).toHaveBeenCalledWith('list_directory', { path: '/projects/beta' });
    const names = getFileNames(fileDropdown);
    expect(names).toContain('lib/');
    expect(names).toContain('main.rs');
  });
});
