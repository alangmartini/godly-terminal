/** Quote a file path if it contains spaces, for pasting into the terminal. */
export function quotePath(path: string): string {
  if (path.includes(' ')) {
    return `"${path}"`;
  }
  return path;
}
