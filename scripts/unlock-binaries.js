// Pre-build script: rename locked .exe files so cargo can write fresh ones.
// On Windows, running executables can't be overwritten but CAN be renamed.
// The process continues running from the renamed file (mapped by handle, not name).

import { rename, unlink } from 'fs/promises';
import { join } from 'path';

const BINARIES = ['godly-daemon.exe', 'godly-mcp.exe', 'godly-notify.exe'];
const TARGET_DIR = join(import.meta.dirname, '..', 'src-tauri', 'target');
const PROFILES = ['debug', 'release'];

async function unlockBinary(filePath) {
  const oldPath = filePath + '.old';

  // Delete leftover .old file from previous build
  try {
    await unlink(oldPath);
  } catch {
    // Doesn't exist or still locked — ignore
  }

  // Rename the locked binary so cargo can write a new one
  try {
    await rename(filePath, oldPath);
  } catch {
    // File doesn't exist or isn't locked — ignore
  }
}

async function main() {
  const tasks = [];
  for (const profile of PROFILES) {
    for (const binary of BINARIES) {
      tasks.push(unlockBinary(join(TARGET_DIR, profile, binary)));
    }
  }
  await Promise.all(tasks);
}

main();
