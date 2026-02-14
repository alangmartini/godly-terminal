import { invoke } from '@tauri-apps/api/core';
import { marked } from 'marked';
import DOMPurify from 'dompurify';

marked.setOptions({ gfm: true, breaks: true });

/** Render markdown to sanitized HTML. Exported for testing. */
export function renderMarkdown(src: string): string {
  const raw = marked.parse(src) as string;
  return DOMPurify.sanitize(raw);
}

/**
 * Show a file editor dialog for editing a text file (e.g. CLAUDE.md).
 * Auto-creates the file if it doesn't exist (starts with empty content).
 */
export async function showFileEditorDialog(title: string, filePath: string): Promise<void> {
  // Read current content (returns empty string if file doesn't exist)
  const content = await invoke<string>('read_file', { path: filePath });

  return new Promise((resolve) => {
    const overlay = document.createElement('div');
    overlay.className = 'dialog-overlay';

    const dialog = document.createElement('div');
    dialog.className = 'dialog file-editor-dialog';

    const titleEl = document.createElement('div');
    titleEl.className = 'dialog-title';
    titleEl.textContent = title;
    dialog.appendChild(titleEl);

    const pathEl = document.createElement('div');
    pathEl.className = 'file-editor-path';
    pathEl.textContent = filePath;
    pathEl.title = filePath;
    dialog.appendChild(pathEl);

    // Split container: textarea (left) + preview (right)
    const split = document.createElement('div');
    split.className = 'file-editor-split';

    const textarea = document.createElement('textarea');
    textarea.className = 'file-editor-textarea';
    textarea.value = content;
    textarea.spellcheck = false;

    const preview = document.createElement('div');
    preview.className = 'file-editor-preview';
    preview.textContent = '';
    updatePreview(preview, content);

    split.appendChild(textarea);
    split.appendChild(preview);
    dialog.appendChild(split);

    // Update preview on every input
    textarea.addEventListener('input', () => {
      updatePreview(preview, textarea.value);
    });

    const buttons = document.createElement('div');
    buttons.className = 'dialog-buttons';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'dialog-btn dialog-btn-secondary';
    cancelBtn.textContent = 'Cancel';

    const saveBtn = document.createElement('button');
    saveBtn.className = 'dialog-btn dialog-btn-primary';
    saveBtn.textContent = 'Save';

    buttons.appendChild(cancelBtn);
    buttons.appendChild(saveBtn);
    dialog.appendChild(buttons);

    const close = () => {
      overlay.remove();
      resolve();
    };

    cancelBtn.onclick = close;

    saveBtn.onclick = async () => {
      try {
        await invoke('write_file', { path: filePath, content: textarea.value });
        close();
      } catch (err) {
        console.error('Failed to save file:', err);
      }
    };

    // Escape to close, Ctrl+S to save
    textarea.onkeydown = (e) => {
      if (e.key === 'Escape') {
        close();
      } else if (e.key === 's' && (e.ctrlKey || e.metaKey)) {
        e.preventDefault();
        saveBtn.click();
      }
    };

    overlay.onclick = (e) => {
      if (e.target === overlay) close();
    };

    overlay.appendChild(dialog);
    document.body.appendChild(overlay);
    textarea.focus();
  });
}

function updatePreview(el: HTMLElement, markdown: string): void {
  const sanitized = renderMarkdown(markdown);
  // Safe: content is sanitized by DOMPurify
  el.replaceChildren();
  const template = document.createElement('template');
  template.innerHTML = sanitized;
  el.appendChild(template.content);
}
