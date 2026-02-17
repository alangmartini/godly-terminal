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
