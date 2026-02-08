import { describe, it, expect } from 'vitest';
import { isAppShortcut } from './keyboard';

function keydown(key: string, opts: { ctrlKey?: boolean; shiftKey?: boolean } = {}) {
  return {
    key,
    type: 'keydown',
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
  };
}

function keyup(key: string, opts: { ctrlKey?: boolean; shiftKey?: boolean } = {}) {
  return {
    key,
    type: 'keyup',
    ctrlKey: opts.ctrlKey ?? false,
    shiftKey: opts.shiftKey ?? false,
  };
}

describe('isAppShortcut', () => {
  // Bug: keyboard shortcuts stopped working when text was selected in the terminal
  // because xterm.js consumed the keydown event as terminal input (e.g. Ctrl+T = ASCII DC4).

  it('returns true for Ctrl+T (new terminal)', () => {
    expect(isAppShortcut(keydown('t', { ctrlKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+W (close terminal)', () => {
    expect(isAppShortcut(keydown('w', { ctrlKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Tab (next tab)', () => {
    expect(isAppShortcut(keydown('Tab', { ctrlKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+Tab (previous tab)', () => {
    expect(isAppShortcut(keydown('Tab', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+S (manual save)', () => {
    expect(isAppShortcut(keydown('S', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+L (manual load)', () => {
    expect(isAppShortcut(keydown('L', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns true for Ctrl+Shift+C (copy selection)', () => {
    expect(isAppShortcut(keydown('C', { ctrlKey: true, shiftKey: true }))).toBe(true);
  });

  it('returns false for Ctrl+C (terminal interrupt — must pass through)', () => {
    expect(isAppShortcut(keydown('c', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for Ctrl+D (terminal EOF)', () => {
    expect(isAppShortcut(keydown('d', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for Ctrl+L (terminal clear)', () => {
    expect(isAppShortcut(keydown('l', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for plain character keys', () => {
    expect(isAppShortcut(keydown('a'))).toBe(false);
    expect(isAppShortcut(keydown('t'))).toBe(false);
  });

  it('returns false for keyup events (only keydown matters)', () => {
    expect(isAppShortcut(keyup('t', { ctrlKey: true }))).toBe(false);
    expect(isAppShortcut(keyup('w', { ctrlKey: true }))).toBe(false);
    expect(isAppShortcut(keyup('Tab', { ctrlKey: true }))).toBe(false);
  });

  it('returns false for Shift+key without Ctrl', () => {
    expect(isAppShortcut(keydown('S', { shiftKey: true }))).toBe(false);
    expect(isAppShortcut(keydown('T', { shiftKey: true }))).toBe(false);
  });
});
