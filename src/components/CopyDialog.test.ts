import { describe, it, expect } from 'vitest';
import { cleanTerminalText } from './CopyDialog';

describe('cleanTerminalText', () => {
  it('trims trailing whitespace per line', () => {
    const input = 'hello   \nworld  \n';
    expect(cleanTerminalText(input)).toBe('hello\nworld');
  });

  it('collapses 3+ consecutive blank lines into 2', () => {
    const input = 'a\n\n\n\n\nb';
    expect(cleanTerminalText(input)).toBe('a\n\n\nb');
  });

  it('preserves exactly 2 consecutive blank lines', () => {
    const input = 'a\n\n\nb';
    expect(cleanTerminalText(input)).toBe('a\n\n\nb');
  });

  it('strips leading blank lines', () => {
    const input = '\n\n\nhello';
    expect(cleanTerminalText(input)).toBe('hello');
  });

  it('strips trailing blank lines', () => {
    const input = 'hello\n\n\n';
    expect(cleanTerminalText(input)).toBe('hello');
  });

  it('strips both leading and trailing blank lines', () => {
    const input = '\n\nhello\nworld\n\n';
    expect(cleanTerminalText(input)).toBe('hello\nworld');
  });

  it('returns empty string for empty input', () => {
    expect(cleanTerminalText('')).toBe('');
  });

  it('returns empty string for whitespace-only input', () => {
    expect(cleanTerminalText('   \n   \n   ')).toBe('');
  });

  it('handles single line without trailing whitespace', () => {
    expect(cleanTerminalText('hello')).toBe('hello');
  });

  it('handles single line with trailing whitespace', () => {
    expect(cleanTerminalText('hello   ')).toBe('hello');
  });

  it('preserves leading indentation', () => {
    const input = '  function foo() {\n    return 1;\n  }';
    expect(cleanTerminalText(input)).toBe('  function foo() {\n    return 1;\n  }');
  });

  it('preserves a single blank line between paragraphs', () => {
    const input = 'paragraph one\n\nparagraph two';
    expect(cleanTerminalText(input)).toBe('paragraph one\n\nparagraph two');
  });

  it('handles terminal-padded lines with mixed content', () => {
    // Simulates typical terminal output: lines padded to 80 cols with spaces
    const input = 'ls -la                                                                          \ntotal 42                                                                        \ndrwxr-xr-x  5 user user 4096 Jan 01 12:00 .                                     \n';
    const result = cleanTerminalText(input);
    expect(result).toBe('ls -la\ntotal 42\ndrwxr-xr-x  5 user user 4096 Jan 01 12:00 .');
  });

  it('collapses multiple groups of excessive blank lines', () => {
    const input = 'a\n\n\n\n\nb\n\n\n\nc';
    expect(cleanTerminalText(input)).toBe('a\n\n\nb\n\n\nc');
  });
});
