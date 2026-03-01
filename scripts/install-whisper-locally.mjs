#!/usr/bin/env node
/**
 * Dev convenience script: copies the locally-built godly-whisper.exe
 * to %LOCALAPPDATA%/godly-whisper/ so the main app can discover it.
 *
 * Usage:  npm run install:whisper
 *         node scripts/install-whisper-locally.mjs [--release]
 */

import { existsSync, mkdirSync, copyFileSync, writeFileSync } from 'fs';
import { join } from 'path';
import { execSync } from 'child_process';

const isRelease = process.argv.includes('--release');
const profile = isRelease ? 'release' : 'debug';
const src = join('src-tauri', 'target', profile, 'godly-whisper.exe');

if (!existsSync(src)) {
  console.error(`Binary not found: ${src}`);
  console.error(`Run 'npm run build:whisper${isRelease ? ':release' : ''}' first.`);
  process.exit(1);
}

const localAppData = process.env.LOCALAPPDATA;
if (!localAppData) {
  console.error('LOCALAPPDATA environment variable not set');
  process.exit(1);
}

const destDir = join(localAppData, 'godly-whisper');
mkdirSync(destDir, { recursive: true });

const dest = join(destDir, 'godly-whisper.exe');
copyFileSync(src, dest);
console.log(`Copied ${src} -> ${dest}`);

// Generate version.json from the binary's --version output
try {
  const versionJson = execSync(`"${dest}" --version`, { encoding: 'utf8' }).trim();
  const versionFile = join(destDir, 'version.json');
  writeFileSync(versionFile, versionJson + '\n');
  console.log(`Wrote ${versionFile}: ${versionJson}`);
} catch {
  console.warn('Warning: could not generate version.json (--version flag may not be supported)');
}

console.log('Done. Godly Terminal will now detect the whisper binary.');
