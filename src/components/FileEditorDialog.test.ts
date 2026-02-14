// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { renderMarkdown } from './FileEditorDialog';

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
