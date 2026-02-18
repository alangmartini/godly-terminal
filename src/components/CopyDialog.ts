/**
 * Clean up terminal text and present it in an editable dialog for copying.
 */

/**
 * Clean terminal text by removing formatting artifacts:
 * 1. Trim trailing whitespace per line (terminal rows are space-padded)
 * 2. Collapse 3+ consecutive blank lines into 2
 * 3. Strip leading/trailing blank lines from the whole block
 */
export function cleanTerminalText(raw: string): string {
  // Trim trailing whitespace per line
  const lines = raw.split('\n').map((line) => line.trimEnd());

  // Collapse 3+ consecutive blank lines into 2
  const collapsed: string[] = [];
  let consecutiveBlanks = 0;
  for (const line of lines) {
    if (line === '') {
      consecutiveBlanks++;
      if (consecutiveBlanks <= 2) {
        collapsed.push(line);
      }
    } else {
      consecutiveBlanks = 0;
      collapsed.push(line);
    }
  }

  // Strip leading blank lines
  while (collapsed.length > 0 && collapsed[0] === '') {
    collapsed.shift();
  }
  // Strip trailing blank lines
  while (collapsed.length > 0 && collapsed[collapsed.length - 1] === '') {
    collapsed.pop();
  }

  return collapsed.join('\n');
}

/**
 * Show a dialog with cleaned terminal text for editing and copying.
 * Follows the existing dialog pattern from dialogs.ts.
 */
export function showCopyDialog(text: string): Promise<void> {
  const cleaned = cleanTerminalText(text);

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog copy-dialog';

    const title = document.createElement('div');
    title.className = 'dialog-title';
    title.textContent = 'Copy Text';
    dialog.appendChild(title);

    const subtitle = document.createElement('div');
    subtitle.className = 'copy-dialog-subtitle';
    subtitle.textContent = 'Edit the text below, then copy to clipboard.';
    dialog.appendChild(subtitle);

    const textarea = document.createElement('textarea');
    textarea.className = 'copy-dialog-textarea';
    textarea.value = cleaned;
    textarea.spellcheck = false;
    dialog.appendChild(textarea);

    const footer = document.createElement('div');
    footer.className = 'copy-dialog-footer';

    const lineCount = document.createElement('span');
    lineCount.className = 'copy-dialog-line-count';
    const updateLineCount = () => {
      const count = textarea.value.split('\n').length;
      lineCount.textContent = `${count} line${count !== 1 ? 's' : ''}`;
    };
    updateLineCount();
    textarea.addEventListener('input', updateLineCount);
    footer.appendChild(lineCount);

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';
    buttons.appendChild(cancelBtn);

    const copyBtn = document.createElement('button');
    copyBtn.className = 'dialog-btn dialog-btn-primary';
    copyBtn.textContent = 'Copy';
    buttons.appendChild(copyBtn);

    footer.appendChild(buttons);
    dialog.appendChild(footer);
    overlay.appendChild(dialog);

    const close = () => {
      overlay.remove();
      resolve();
    };

    const copyAndClose = () => {
      navigator.clipboard.writeText(textarea.value);
      close();
    };

    cancelBtn.onclick = close;
    copyBtn.onclick = copyAndClose;

    textarea.addEventListener('keydown', (e) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        close();
      }
      if (e.key === 'Enter' && e.ctrlKey) {
        e.preventDefault();
        copyAndClose();
      }
    });

    overlay.onclick = (e) => {
      if (e.target === overlay) {
        close();
      }
    };

    document.body.appendChild(overlay);
    textarea.focus();
    textarea.select();
  });
}
