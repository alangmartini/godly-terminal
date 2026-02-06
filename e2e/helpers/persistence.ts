/**
 * Helpers for reading / clearing persistence files on disk.
 */

import fs from 'fs';
import path from 'path';
import os from 'os';

function getAppDataDir(): string {
  const appData =
    process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
  return path.join(appData, 'com.godly.terminal');
}

export function getLayoutPath(): string {
  return path.join(getAppDataDir(), 'layout.json');
}

export function getScrollbackDir(): string {
  return path.join(getAppDataDir(), 'scrollback');
}

/**
 * Delete layout.json and the scrollback/ directory so tests start clean.
 */
export function clearAppData(): void {
  const layoutPath = getLayoutPath();
  if (fs.existsSync(layoutPath)) {
    fs.unlinkSync(layoutPath);
  }

  const scrollbackDir = getScrollbackDir();
  if (fs.existsSync(scrollbackDir)) {
    fs.rmSync(scrollbackDir, { recursive: true, force: true });
  }
}

/**
 * Read and parse the persisted layout file.
 * Returns null if the file doesn't exist.
 */
export function readLayoutFile(): any | null {
  const layoutPath = getLayoutPath();
  if (!fs.existsSync(layoutPath)) return null;
  const raw = fs.readFileSync(layoutPath, 'utf-8');
  return JSON.parse(raw);
}

/**
 * List scrollback .dat files.
 */
export function getScrollbackFiles(): string[] {
  const dir = getScrollbackDir();
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith('.dat'));
}
