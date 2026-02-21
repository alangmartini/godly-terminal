/**
 * Simple glob matching: converts a glob pattern to a regex.
 * Supports `*` (any characters) and `?` (single character).
 * Case-insensitive.
 */
export function globMatch(pattern: string, text: string): boolean {
  // Escape regex special chars except * and ?
  const escaped = pattern.replace(/[.+^${}()|[\]\\]/g, '\\$&');
  // Convert glob wildcards to regex
  const regexStr = '^' + escaped.replace(/\*/g, '.*').replace(/\?/g, '.') + '$';
  return new RegExp(regexStr, 'i').test(text);
}
