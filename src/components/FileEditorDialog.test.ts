// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderMarkdown, showFileEditorDialog } from './FileEditorDialog';

describe('renderMarkdown', () => {
  it('renders basic markdown to HTML', () => {
    const html = renderMarkdown('# Hello');
    expect(html).toContain('<h1');
    expect(html).toContain('Hello');
  });

  it('renders GFM tables', () => {
    const md = `| Col A | Col B |\n|-------|-------|\n| 1     | 2     |`;
    const html = renderMarkdown(md);
    expect(html).toContain('<table');
    expect(html).toContain('<th');
    expect(html).toContain('Col A');
    expect(html).toContain('<td');
    expect(html).toContain('1');
  });

  it('renders fenced code blocks', () => {
    const md = '```js\nconsole.log("hi");\n```';
    const html = renderMarkdown(md);
    expect(html).toContain('<pre');
    expect(html).toContain('<code');
    expect(html).toContain('console.log');
  });

  it('renders line breaks with breaks:true', () => {
    const md = 'line one\nline two';
    const html = renderMarkdown(md);
    expect(html).toContain('<br');
  });

  it('sanitizes script tags via DOMPurify', () => {
    const md = '<script>alert("xss")</script>';
    const html = renderMarkdown(md);
    expect(html).not.toContain('<script');
  });

  it('returns empty string for empty input', () => {
    const html = renderMarkdown('');
    expect(html).toBe('');
  });
});

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

describe('showFileEditorDialog with defaultContent', () => {
  beforeEach(() => {
    document.body.textContent = '';
  });

  it('uses defaultContent when file is empty (new file)', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const mockInvoke = vi.mocked(invoke);
    // read_file returns empty string for non-existent file
    mockInvoke.mockResolvedValueOnce('');

    const defaultContent = '@echo off\ndoskey dclaude=claude --dangerously-skip-permissions $*\n';
    // Don't await — dialog is modal, we need to inspect it while open
    const dialogPromise = showFileEditorDialog('CMD Aliases', 'C:\\test\\cmd-aliases.cmd', defaultContent);

    // Wait for the dialog to render
    await new Promise(r => setTimeout(r, 50));

    const textarea = document.querySelector('.file-editor-textarea') as HTMLTextAreaElement;
    expect(textarea).toBeTruthy();
    expect(textarea.value).toBe(defaultContent);

    // Close the dialog via cancel
    const cancelBtn = document.querySelector('.dialog-btn-secondary') as HTMLButtonElement;
    cancelBtn?.click();
    await dialogPromise;
  });

  it('uses existing file content when file is not empty', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const mockInvoke = vi.mocked(invoke);
    const existingContent = '@echo off\ndoskey myalias=echo hello $*\n';
    mockInvoke.mockResolvedValueOnce(existingContent);

    const defaultContent = '@echo off\ndoskey dclaude=claude --dangerously-skip-permissions $*\n';
    const dialogPromise = showFileEditorDialog('CMD Aliases', 'C:\\test\\cmd-aliases.cmd', defaultContent);

    await new Promise(r => setTimeout(r, 50));

    const textarea = document.querySelector('.file-editor-textarea') as HTMLTextAreaElement;
    expect(textarea).toBeTruthy();
    // Should use existing content, not default
    expect(textarea.value).toBe(existingContent);

    const cancelBtn = document.querySelector('.dialog-btn-secondary') as HTMLButtonElement;
    cancelBtn?.click();
    await dialogPromise;
  });

  it('does not apply defaultContent when not provided', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const mockInvoke = vi.mocked(invoke);
    mockInvoke.mockResolvedValueOnce('');

    const dialogPromise = showFileEditorDialog('Test', 'C:\\test\\empty.md');

    await new Promise(r => setTimeout(r, 50));

    const textarea = document.querySelector('.file-editor-textarea') as HTMLTextAreaElement;
    expect(textarea).toBeTruthy();
    expect(textarea.value).toBe('');

    const cancelBtn = document.querySelector('.dialog-btn-secondary') as HTMLButtonElement;
    cancelBtn?.click();
    await dialogPromise;
  });
});
