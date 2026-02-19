/**
 * Show a prompt dialog for entering a custom worktree branch name.
 * Returns the user's input (empty string = auto-generate), or null if cancelled.
 */
export function showWorktreeNamePrompt(): Promise<string | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'New Worktree Branch';
    dialog.appendChild(title);

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'dialog-input';
    input.placeholder = 'Branch name (Enter for auto-generated)';
    dialog.appendChild(input);

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    buttons.appendChild(cancelBtn);

    const okBtn = document.createElement('button');
    okBtn.className = 'dialog-btn dialog-btn-primary';
    okBtn.textContent = 'Create';
    buttons.appendChild(okBtn);

    dialog.appendChild(buttons);
    overlay.appendChild(dialog);

    const close = () => overlay.remove();

    cancelBtn.onclick = () => {
      close();
      resolve(null);
    };

    okBtn.onclick = () => {
      close();
      resolve(input.value.trim());
    };

    input.onkeydown = (e) => {
      if (e.key === 'Enter') {
        close();
        resolve(input.value.trim());
      }
      if (e.key === 'Escape') {
        close();
        resolve(null);
      }
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) {
        close();
        resolve(null);
      }
    };

    document.body.appendChild(overlay);
    input.focus();
  });
}

/**
 * Show a prompt dialog for entering a Figma file URL.
 * Returns the URL string, or null if cancelled.
 */
export function showFigmaUrlPrompt(): Promise<string | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'Open Figma Design';
    dialog.appendChild(title);

    const hint = document.createElement('div');
    hint.style.cssText = 'font-size: 12px; color: var(--text-secondary); margin-bottom: 12px;';
    hint.textContent = 'Paste a Figma file URL (e.g. https://figma.com/design/...)';
    dialog.appendChild(hint);

    const input = document.createElement('input');
    input.type = 'text';
    input.className = 'dialog-input';
    input.placeholder = 'https://figma.com/design/...';
    dialog.appendChild(input);

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    buttons.appendChild(cancelBtn);

    const okBtn = document.createElement('button');
    okBtn.className = 'dialog-btn dialog-btn-primary';
    okBtn.textContent = 'Open';
    buttons.appendChild(okBtn);

    dialog.appendChild(buttons);
    overlay.appendChild(dialog);

    const close = () => overlay.remove();

    const submit = () => {
      const url = input.value.trim();
      close();
      if (url && url.includes('figma.com')) {
        resolve(url);
      } else if (url) {
        // Not a valid Figma URL
        resolve(null);
      } else {
        resolve(null);
      }
    };

    cancelBtn.onclick = () => {
      close();
      resolve(null);
    };

    okBtn.onclick = submit;

    input.onkeydown = (e) => {
      if (e.key === 'Enter') submit();
      if (e.key === 'Escape') {
        close();
        resolve(null);
      }
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) {
        close();
        resolve(null);
      }
    };

    document.body.appendChild(overlay);
    input.focus();
  });
}

/**
 * Quick Claude dialog: capture an idea to dispatch to a new Claude Code session.
 * Returns { prompt, branchName? } or null if cancelled.
 */
export interface QuickClaudeInput {
  prompt: string;
  branchName?: string;
  workspaceId: string;
}

export interface QuickClaudeOptions {
  workspaces: { id: string; name: string }[];
  activeWorkspaceId: string;
}

const QUICK_CLAUDE_WORKSPACE_KEY = 'quick-claude-last-workspace';

export function showQuickClaudeDialog(options: QuickClaudeOptions): Promise<QuickClaudeInput | null> {
  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'Quick Claude';
    dialog.appendChild(title);

    const hint = document.createElement('div');
    hint.style.cssText = 'font-size: 12px; color: var(--text-secondary); margin-bottom: 8px;';
    hint.textContent = 'Ctrl+Enter to launch \u00b7 Escape to cancel';
    dialog.appendChild(hint);

    const workspaceSelect = document.createElement('select');
    workspaceSelect.className = 'dialog-input';
    workspaceSelect.style.marginBottom = '8px';
    for (const ws of options.workspaces) {
      const opt = document.createElement('option');
      opt.value = ws.id;
      opt.textContent = ws.name;
      workspaceSelect.appendChild(opt);
    }
    const savedId = localStorage.getItem(QUICK_CLAUDE_WORKSPACE_KEY);
    const validSaved = savedId && options.workspaces.some(ws => ws.id === savedId);
    workspaceSelect.value = validSaved ? savedId : options.activeWorkspaceId;
    dialog.appendChild(workspaceSelect);

    const promptArea = document.createElement('textarea');
    promptArea.className = 'dialog-input';
    promptArea.placeholder = 'Describe your idea...';
    promptArea.rows = 4;
    promptArea.style.cssText = 'resize: vertical; min-height: 80px; font-family: inherit; font-size: 13px;';
    dialog.appendChild(promptArea);

    const branchInput = document.createElement('input');
    branchInput.type = 'text';
    branchInput.className = 'dialog-input';
    branchInput.placeholder = 'Branch name (optional, auto-generated if empty)';
    branchInput.style.marginTop = '8px';
    dialog.appendChild(branchInput);

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    buttons.appendChild(cancelBtn);

    const okBtn = document.createElement('button');
    okBtn.className = 'dialog-btn dialog-btn-primary';
    okBtn.textContent = 'Launch';
    buttons.appendChild(okBtn);

    dialog.appendChild(buttons);
    overlay.appendChild(dialog);

    const close = () => overlay.remove();

    const submit = () => {
      const prompt = promptArea.value.trim();
      if (!prompt) return;
      localStorage.setItem(QUICK_CLAUDE_WORKSPACE_KEY, workspaceSelect.value);
      close();
      resolve({
        prompt,
        branchName: branchInput.value.trim() || undefined,
        workspaceId: workspaceSelect.value,
      });
    };

    cancelBtn.onclick = () => { close(); resolve(null); };
    okBtn.onclick = submit;

    promptArea.onkeydown = (e) => {
      if (e.key === 'Enter' && e.ctrlKey) { e.preventDefault(); submit(); }
      if (e.key === 'Escape') { close(); resolve(null); }
    };

    branchInput.onkeydown = (e) => {
      if (e.key === 'Enter') { e.preventDefault(); submit(); }
      if (e.key === 'Escape') { close(); resolve(null); }
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) { close(); resolve(null); }
    };

    document.body.appendChild(overlay);
    promptArea.focus();
  });
}
