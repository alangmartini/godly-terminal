import { describe, it, expect } from 'vitest';
import { quotePath } from './quote-path';

describe('quotePath', () => {
  it('should return path unchanged when it has no spaces', () => {
    expect(quotePath('C:\\Users\\test\\file.png')).toBe('C:\\Users\\test\\file.png');
  });

  it('should wrap path in double quotes when it contains spaces', () => {
    // Bug trigger: ShareX screenshots land in paths like "C:\Users\name\My Screenshots\image.png"
    expect(quotePath('C:\\Users\\test\\My Screenshots\\image.png'))
      .toBe('"C:\\Users\\test\\My Screenshots\\image.png"');
  });

  it('should handle single-segment path with space', () => {
    expect(quotePath('my file.txt')).toBe('"my file.txt"');
  });

  it('should handle multiple paths joined with space for multi-file drop', () => {
    const paths = [
      'C:\\no-space.txt',
      'C:\\has space\\file.png',
      'D:\\another path\\doc.pdf',
    ];
    const result = paths.map(quotePath).join(' ');
    expect(result).toBe('C:\\no-space.txt "C:\\has space\\file.png" "D:\\another path\\doc.pdf"');
  });
});
