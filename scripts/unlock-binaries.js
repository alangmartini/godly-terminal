// Pre-build script: rename locked .exe files so cargo can write fresh ones.
// On Windows, running executables can't be overwritten but CAN be renamed.
// The process continues running from the renamed file (mapped by handle, not name).

import { mkdir, rename, unlink, writeFile, stat } from 'fs/promises';
import { join } from 'path';

const BINARIES = ['godly-daemon.exe', 'godly-mcp.exe', 'godly-notify.exe', 'godly-terminal.exe'];
// Binaries referenced in tauri.conf.json bundle.resources (release profile only)
const RESOURCE_BINARIES = ['godly-daemon.exe', 'godly-mcp.exe', 'godly-notify.exe'];
const TARGET_DIR = join(import.meta.dirname, '..', 'src-tauri', 'target');
// Only unlock the requested profile. Default to 'debug' to avoid destroying
// release binaries that Tauri's build.rs needs for resource path validation.
// Pass --release or --all via: npm run unlock -- --release
const args = process.argv.slice(2);
const PROFILES = args.includes('--all')
  ? ['debug', 'release']
  : args.includes('--release')
    ? ['release']
    : ['debug'];

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

async function ensureReleaseStubs() {
  // tauri_build::build() validates bundle.resources paths at compile time,
  // even in dev mode. Create empty stubs so dev builds don't fail when
  // release binaries haven't been built yet.
  const releaseDir = join(TARGET_DIR, 'release');
  await mkdir(releaseDir, { recursive: true });
  for (const binary of RESOURCE_BINARIES) {
    const filePath = join(releaseDir, binary);
    try {
      await stat(filePath);
    } catch {
      await writeFile(filePath, '');
    }
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
  await ensureReleaseStubs();
}

main();
