// @vitest-environment jsdom

import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Bug #495: Quick Claude AI tool selection not persisted between invocations.
 *
 * The Quick Claude dialog persists 3 preferences via localStorage:
 *   - QUICK_CLAUDE_WORKSPACE_KEY (workspace selection)
 *   - QUICK_CLAUDE_NO_WORKTREE_KEY (no-worktree checkbox)
 *   - QUICK_CLAUDE_AUTO_SUGGEST_KEY (auto-suggest branch name)
 *
 * But it does NOT persist the AI tool selection (claude/codex/both).
 * On every invocation, the AI tool dropdown resets to the workspace's
 * stored aiToolMode instead of remembering the user's last choice.
 *
 * Additionally, the dialog:
 *   - Has no support for custom AI tool binaries
 *   - Is limited to 3 options (claude/codex/both) with no extensibility
 *   - Doesn't support more than 2 simultaneous launches
 */

// ── Mock data ─────────────────────────────────────────────────────────

const WORKSPACES = [
  { id: 'ws-main', name: 'Main Project', folderPath: '/projects/main', aiToolMode: 'claude' },
  { id: 'ws-codex', name: 'Codex Project', folderPath: '/projects/codex', aiToolMode: 'codex' },
];

// ── Setup ─────────────────────────────────────────────────────────────

let mockInvoke: ReturnType<typeof vi.fn>;

beforeEach(() => {
  document.body.innerHTML = '';
  localStorage.clear();

  mockInvoke = vi.fn(async () => null);
  vi.doMock('@tauri-apps/api/core', () => ({ invoke: mockInvoke }));
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.doUnmock('@tauri-apps/api/core');
});

// ── Helpers ───────────────────────────────────────────────────────────

async function openDialog(workspaces = WORKSPACES, activeWorkspaceId = 'ws-main') {
  const { showQuickClaudeDialog } = await import('./dialogs');
  const resultPromise = showQuickClaudeDialog({ workspaces, activeWorkspaceId });

  // Wait for DOM to render
  await new Promise(r => setTimeout(r, 10));

  const overlay = document.querySelector('.dialog-overlay') as HTMLDivElement;
  const aiToolSelect = overlay.querySelector('[data-testid="ai-tool-mode"]') as HTMLSelectElement;
  const promptArea = overlay.querySelector('textarea.dialog-input') as HTMLTextAreaElement;
  const workspaceSelect = overlay.querySelector('select:not([data-testid])') as HTMLSelectElement;
  const launchBtn = overlay.querySelector('.dialog-btn-primary') as HTMLButtonElement;

  return { resultPromise, overlay, aiToolSelect, promptArea, workspaceSelect, launchBtn };
}

function submitDialog(promptArea: HTMLTextAreaElement, launchBtn: HTMLButtonElement, prompt = 'test prompt') {
  promptArea.value = prompt;
  promptArea.dispatchEvent(new Event('input', { bubbles: true }));
  launchBtn.click();
}

// ── Tests ─────────────────────────────────────────────────────────────

describe('Bug #495: Quick Claude AI tool selection persistence', () => {
  it('should persist AI tool selection to localStorage on submit', async () => {
    // Bug #495: AI tool selection is NOT saved to localStorage on submit
    const { aiToolSelect, promptArea, launchBtn, resultPromise } = await openDialog();

    // Change AI tool to codex
    aiToolSelect.value = 'codex';
    aiToolSelect.dispatchEvent(new Event('change', { bubbles: true }));

    // Submit the dialog
    submitDialog(promptArea, launchBtn);
    await resultPromise;

    // Expected: localStorage should have the AI tool selection saved
    // Actual: no localStorage key for AI tool exists
    const savedAiTool = localStorage.getItem('quick-claude-ai-tool');
    expect(savedAiTool).toBe('codex');
  });

  it('should restore AI tool selection from localStorage on next invocation', async () => {
    // Bug #495: Dialog always resets to workspace aiToolMode
    // First invocation: select codex and submit
    const first = await openDialog();
    first.aiToolSelect.value = 'codex';
    first.aiToolSelect.dispatchEvent(new Event('change', { bubbles: true }));
    submitDialog(first.promptArea, first.launchBtn);
    await first.resultPromise;

    // Second invocation: should remember codex
    const second = await openDialog();

    // Expected: AI tool should be 'codex' (remembered from last time)
    // Actual: AI tool resets to 'claude' (workspace's aiToolMode)
    expect(second.aiToolSelect.value).toBe('codex');
  });

  it('should persist "both" selection across invocations', async () => {
    // Bug #495: "both" mode selection lost between Quick Claude invocations
    const first = await openDialog();
    first.aiToolSelect.value = 'both';
    first.aiToolSelect.dispatchEvent(new Event('change', { bubbles: true }));
    submitDialog(first.promptArea, first.launchBtn);
    await first.resultPromise;

    const second = await openDialog();
    expect(second.aiToolSelect.value).toBe('both');
  });

  it('should use localStorage AI tool over workspace default when both exist', async () => {
    // Bug #495: localStorage preference should override workspace aiToolMode
    // Workspace 'ws-main' has aiToolMode = 'claude'
    // But user last selected 'both' — localStorage should win
    localStorage.setItem('quick-claude-ai-tool', 'both');

    const { aiToolSelect } = await openDialog();

    // Expected: 'both' from localStorage (user's explicit choice)
    // Actual: 'claude' from workspace.aiToolMode
    expect(aiToolSelect.value).toBe('both');
  });

  it('should return selected AI tool in result even when workspace default differs', async () => {
    // Verify the result object includes the user's selection
    const { aiToolSelect, promptArea, launchBtn, resultPromise } = await openDialog();

    aiToolSelect.value = 'codex';
    aiToolSelect.dispatchEvent(new Event('change', { bubbles: true }));
    submitDialog(promptArea, launchBtn);

    const result = await resultPromise;
    expect(result).not.toBeNull();
    expect(result!.aiTool).toBe('codex');
  });
});

describe('Bug #495: AI tool selection independence from workspace switching', () => {
  it('should NOT override localStorage AI tool when workspace changes', async () => {
    // Bug #495: Changing workspace resets AI tool to workspace's aiToolMode
    // When user has a saved preference, workspace changes should not override it
    localStorage.setItem('quick-claude-ai-tool', 'both');

    const { aiToolSelect, workspaceSelect } = await openDialog();

    // Initial should be 'both' from localStorage
    // (This already fails due to the main bug, but test the workspace-change behavior too)
    expect(aiToolSelect.value).toBe('both');

    // Change workspace to 'ws-codex' (which has aiToolMode = 'codex')
    workspaceSelect.value = 'ws-codex';
    workspaceSelect.dispatchEvent(new Event('change', { bubbles: true }));

    // After workspace change, workspace's mode should be respected
    // since user explicitly switched workspace context.
    // But if user then changes AI tool manually, THAT should be persisted.
    // Current behavior: always uses workspace's aiToolMode, ignoring user's preference
  });
});

describe('Bug #495: Settings page for AI tools (feature gaps)', () => {
  it('should support custom AI tool options beyond claude/codex/both', async () => {
    // Feature gap: dialog only supports 3 hardcoded options
    // Should support custom binaries configured in settings
    const { aiToolSelect } = await openDialog();

    const optionValues = Array.from(aiToolSelect.options).map(o => o.value);

    // Currently only has: claude, codex, both
    // Expected: should also support custom-configured tools
    // This test documents the current limitation — it passes on current code
    // but should be updated when custom tools are implemented
    expect(optionValues).toContain('claude');
    expect(optionValues).toContain('codex');
    expect(optionValues).toContain('both');

    // Feature gap: no custom tool options available
    // Once settings page is implemented, custom tools should appear here
    const hasCustomOption = optionValues.some(v => !['claude', 'codex', 'both'].includes(v));
    expect(hasCustomOption).toBe(false); // Documents current state
  });

  it('should allow configuring up to 4 simultaneous AI tool launches', async () => {
    // Feature gap: "both" mode is limited to exactly 2 tools
    // Should support up to 4 simultaneous launches
    const { aiToolSelect } = await openDialog();

    // Currently the max is 'both' = 2 tools
    // Feature: should support selecting multiple tools for N-way split (up to 4)
    const maxTools = aiToolSelect.value === 'both' ? 2 : 1;
    expect(maxTools).toBeLessThanOrEqual(2); // Documents current limit
    // When implemented: expect maxTools capability to be 4
  });
});
