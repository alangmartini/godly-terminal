/**
 * Simple script to check persistence state
 * Run with: npx ts-node e2e/check-persistence.ts
 */

import * as fs from 'fs';
import * as path from 'path';
import * as os from 'os';

// Find the app data directory
const appDataDir = process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
const godlyTerminalDir = path.join(appDataDir, 'com.godly.terminal');
const layoutPath = path.join(godlyTerminalDir, 'layout.json');
const scrollbackDir = path.join(godlyTerminalDir, 'scrollback');

console.log('=== Godly Terminal Persistence Check ===\n');

console.log('Checking paths:');
console.log('  App data dir:', godlyTerminalDir);
console.log('  Layout file:', layoutPath);
console.log('  Scrollback dir:', scrollbackDir);
console.log('');

// Check if layout.json exists
if (fs.existsSync(layoutPath)) {
  console.log('✓ layout.json EXISTS\n');

  const content = fs.readFileSync(layoutPath, 'utf-8');
  const data = JSON.parse(content);

  console.log('Layout content:');
  console.log(JSON.stringify(data, null, 2));
  console.log('');

  // Check the structure
  const layout = data.layout;
  if (layout) {
    console.log('Analysis:');
    console.log('  Workspaces:', layout.workspaces?.length ?? 0);
    console.log('  Terminals:', layout.terminals?.length ?? 0);
    console.log('  Active workspace ID:', layout.active_workspace_id);
    console.log('');

    if (layout.workspaces?.length > 0) {
      console.log('Workspaces:');
      layout.workspaces.forEach((w: any, i: number) => {
        console.log(`  [${i}] id=${w.id}, name="${w.name}", shell_type=${JSON.stringify(w.shell_type)}`);
      });
      console.log('');
    }

    if (layout.terminals?.length > 0) {
      console.log('Terminals:');
      layout.terminals.forEach((t: any, i: number) => {
        console.log(`  [${i}] id=${t.id}, workspace_id=${t.workspace_id}, name="${t.name}"`);
        console.log(`       shell_type=${JSON.stringify(t.shell_type)}, cwd="${t.cwd}"`);
      });
      console.log('');
    }
  }
} else {
  console.log('✗ layout.json DOES NOT EXIST\n');
}

// Check scrollback directory
if (fs.existsSync(scrollbackDir)) {
  console.log('✓ scrollback directory EXISTS\n');

  const files = fs.readdirSync(scrollbackDir);
  console.log('Scrollback files:', files.length);
  files.forEach((f) => {
    const filePath = path.join(scrollbackDir, f);
    const stats = fs.statSync(filePath);
    console.log(`  ${f} (${stats.size} bytes)`);
  });
  console.log('');

  // Check if terminal IDs match
  if (fs.existsSync(layoutPath)) {
    const content = fs.readFileSync(layoutPath, 'utf-8');
    const data = JSON.parse(content);
    const layout = data.layout;

    if (layout?.terminals?.length > 0) {
      console.log('Terminal ID vs Scrollback file matching:');
      layout.terminals.forEach((t: any) => {
        const scrollbackFile = `${t.id}.dat`;
        const exists = files.includes(scrollbackFile);
        console.log(`  ${t.id}: ${exists ? '✓ FOUND' : '✗ NOT FOUND'}`);
      });
    }
  }
} else {
  console.log('✗ scrollback directory DOES NOT EXIST\n');
}

console.log('\n=== End of Check ===');
