// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Tests for Bug #276: Skill autocomplete doesn't refresh when target workspace changes.
 *
 * In the Quick Claude dialog, typing `/` loads skills from the selected workspace.
 * However, changing the workspace dropdown does NOT refresh the autocomplete —
 * it keeps showing skills from the old workspace until the user types again.
 *
 * Root cause: No `change` event listener on the workspace <select> element
 * to re-trigger skill fetch/render when the target workspace changes.
 */

// ── Mock data ────────────────────────────────────────────────────────────

const WORKSPACE_A_SKILLS = [
  { name: 'deploy', description: 'Deploy to prod', usage: '/deploy <env>', source: 'project' },
  { name: 'test-all', description: 'Run all tests', usage: '/test-all', source: 'project' },
];

const WORKSPACE_B_SKILLS = [
  { name: 'build', description: 'Build the project', usage: '/build', source: 'project' },
  { name: 'lint', description: 'Run linter', usage: '/lint', source: 'project' },
];

const WORKSPACES = [
  { id: 'ws-a', name: 'Project Alpha', folderPath: '/projects/alpha' },
  { id: 'ws-b', name: 'Project Beta', folderPath: '/projects/beta' },
];

// ── Setup ────────────────────────────────────────────────────────────────

let mockInvoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  document.body.innerHTML = '';
  localStorage.clear();

  mockInvoke = vi.fn(async (cmd: string, args?: Record<string, unknown>) => {
    if (cmd === 'list_skills') {
      const path = args?.projectPath as string;
      if (path === '/projects/alpha') return WORKSPACE_A_SKILLS;
      if (path === '/projects/beta') return WORKSPACE_B_SKILLS;
      return [];
    }
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
  // Dynamic import to pick up the doMock above
  const { showQuickClaudeDialog } = await import('./dialogs');
  const resultPromise = showQuickClaudeDialog({
    workspaces: WORKSPACES,
    activeWorkspaceId: 'ws-a',
  });

  // Wait for DOM to render
  await vi.dynamicImportSettled?.() ?? new Promise(r => setTimeout(r, 0));

  const overlay = document.querySelector('.dialog-overlay') as HTMLDivElement;
  const promptArea = overlay.querySelector('textarea.dialog-input') as HTMLTextAreaElement;
  const workspaceSelect = overlay.querySelector('select') as HTMLSelectElement;
  const skillDropdown = overlay.querySelector('.skill-dropdown') as HTMLDivElement;

  return { resultPromise, overlay, promptArea, workspaceSelect, skillDropdown };
}

function getSkillNames(dropdown: HTMLDivElement): string[] {
  return Array.from(dropdown.querySelectorAll('.skill-item-name'))
    .map(el => el.textContent ?? '');
}

async function typeSlash(promptArea: HTMLTextAreaElement) {
  promptArea.value = '/';
  promptArea.selectionStart = 1;
  promptArea.selectionEnd = 1;
  promptArea.dispatchEvent(new Event('input', { bubbles: true }));
  // Wait for async fetchSkills + render
  await new Promise(r => setTimeout(r, 50));
}

function changeWorkspace(select: HTMLSelectElement, value: string) {
  select.value = value;
  select.dispatchEvent(new Event('change', { bubbles: true }));
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('Quick Claude skill autocomplete workspace switching (#276)', () => {
  it('shows skills from the initially selected workspace when typing /', async () => {
    const { promptArea, skillDropdown } = await openDialog();

    await typeSlash(promptArea);

    const names = getSkillNames(skillDropdown);
    expect(names).toContain('/deploy');
    expect(names).toContain('/test-all');
    expect(skillDropdown.style.display).not.toBe('none');
  });

  it('refreshes skill dropdown immediately when workspace dropdown changes', async () => {
    // Bug #276: dropdown keeps showing old workspace skills after workspace change
    const { promptArea, workspaceSelect, skillDropdown } = await openDialog();

    // Type / to show workspace A skills
    await typeSlash(promptArea);
    const namesBeforeSwitch = getSkillNames(skillDropdown);
    expect(namesBeforeSwitch).toContain('/deploy');

    // Change to workspace B — this should trigger a re-fetch and re-render
    changeWorkspace(workspaceSelect, 'ws-b');
    await new Promise(r => setTimeout(r, 50));

    // The dropdown should now show workspace B's skills
    const namesAfterSwitch = getSkillNames(skillDropdown);
    expect(namesAfterSwitch).not.toContain('/deploy');
    expect(namesAfterSwitch).not.toContain('/test-all');
    expect(namesAfterSwitch).toContain('/build');
    expect(namesAfterSwitch).toContain('/lint');
  });

  it('fetches skills from the new workspace (not cache) after switching', async () => {
    const { promptArea, workspaceSelect } = await openDialog();

    // Type / to trigger fetch for workspace A
    await typeSlash(promptArea);
    expect(mockInvoke).toHaveBeenCalledWith('list_skills', { projectPath: '/projects/alpha' });

    // Change to workspace B
    changeWorkspace(workspaceSelect, 'ws-b');
    await new Promise(r => setTimeout(r, 50));

    // Should have fetched workspace B's skills
    expect(mockInvoke).toHaveBeenCalledWith('list_skills', { projectPath: '/projects/beta' });
  });

  it('hides dropdown if new workspace has no skills', async () => {
    // Override mock for an empty workspace
    mockInvoke.mockImplementation(async (cmd: string, args?: Record<string, unknown>) => {
      if (cmd === 'list_skills') {
        const path = args?.projectPath as string;
        if (path === '/projects/alpha') return WORKSPACE_A_SKILLS;
        if (path === '/projects/beta') return []; // no skills
        return [];
      }
      return null;
    });

    const { promptArea, workspaceSelect, skillDropdown } = await openDialog();

    await typeSlash(promptArea);
    expect(skillDropdown.style.display).not.toBe('none');

    changeWorkspace(workspaceSelect, 'ws-b');
    await new Promise(r => setTimeout(r, 50));

    // Dropdown should be hidden since workspace B has no skills
    expect(skillDropdown.style.display).toBe('none');
  });

  it('applies current filter query to new workspace skills after switch', async () => {
    const { promptArea, workspaceSelect, skillDropdown } = await openDialog();

    // Type /bu to filter — won't match anything in workspace A
    promptArea.value = '/bu';
    promptArea.selectionStart = 3;
    promptArea.selectionEnd = 3;
    promptArea.dispatchEvent(new Event('input', { bubbles: true }));
    await new Promise(r => setTimeout(r, 50));

    // No match in workspace A — dropdown should be hidden (deploy, test-all don't match "bu")
    expect(getSkillNames(skillDropdown).length).toBe(0);

    // Switch to workspace B which has "build" matching "bu"
    changeWorkspace(workspaceSelect, 'ws-b');
    await new Promise(r => setTimeout(r, 50));

    // Should show 'build' from workspace B filtered by current query 'bu'
    const names = getSkillNames(skillDropdown);
    expect(names).toContain('/build');
    expect(names).not.toContain('/lint');
  });
});
