#!/usr/bin/env node
// scripts/demo-calibrate.mjs
// Interactive calibration for demo recording coordinates.
//
// Usage: node scripts/demo-calibrate.mjs
//
// Hover your mouse over each UI element and press Enter to capture the position.
// Outputs a layout constants object you can paste into demo-acts.mjs.

import { execSync, spawn } from 'child_process';
import { createInterface } from 'readline';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

function findPython() {
  for (const cmd of ['python', 'python3', 'python3.13']) {
    try {
      execSync(`${cmd} -c "import pyautogui"`, { stdio: 'pipe', timeout: 5000 });
      return cmd;
    } catch { /* try next */ }
  }
  const storePy = 'C:/Users/alanm/AppData/Local/Microsoft/WindowsApps/PythonSoftwareFoundation.Python.3.13_qbz5n2kfra8p0/python.exe';
  try {
    execSync(`"${storePy}" -c "import pyautogui"`, { stdio: 'pipe', timeout: 5000 });
    return storePy;
  } catch { /* fall through */ }
  return 'python';
}

// Start gui-bridge
const pythonCmd = findPython();
const py = spawn(pythonCmd, [path.join(__dirname, 'gui-bridge.py')], {
  stdio: ['pipe', 'pipe', 'pipe'],
});

let guiResolve = null;
const rlBridge = createInterface({ input: py.stdout });
rlBridge.on('line', (line) => {
  try {
    const data = JSON.parse(line);
    if (guiResolve) {
      const cb = guiResolve;
      guiResolve = null;
      cb(data);
    }
  } catch {}
});

function gui(cmd) {
  return new Promise((resolve) => {
    guiResolve = resolve;
    py.stdin.write(JSON.stringify(cmd) + '\n');
  });
}

async function getCursor() {
  return gui({ action: 'cursor' });
}

// Interactive prompt
const rl = createInterface({ input: process.stdin, output: process.stdout });
function ask(question) {
  return new Promise((resolve) => rl.question(question, resolve));
}

const points = [
  ['sidebarWs1', 'First workspace in sidebar'],
  ['sidebarWs2', 'Second workspace in sidebar'],
  ['sidebarWs3', 'Third workspace in sidebar'],
  ['sidebarNewWs', 'New workspace "+" button (bottom of sidebar)'],
  ['tab1', 'First tab in tab bar'],
  ['tab2', 'Second tab in tab bar'],
  ['tab3', 'Third tab in tab bar'],
  ['tabAdd', 'New tab "+" button'],
  ['terminal', 'Center of the terminal pane'],
  ['wsNameInput', 'Workspace name input field (open new-ws dialog first)'],
];

console.log('');
console.log('=== Godly Terminal Demo Calibration ===');
console.log('');
console.log('For each element, hover your mouse over it and press Enter.');
console.log('Make sure Godly Terminal is visible and in the position you\'ll use for recording.');
console.log('');
console.log('TIP: Create 3 workspaces with 3 tabs each before starting,');
console.log('     so all UI elements are visible.');
console.log('');

// Wait for bridge ready
await new Promise(r => setTimeout(r, 1000));
await gui({ action: 'ping' });

const layout = {};

for (const [key, label] of points) {
  await ask(`  Hover over: ${label} → then press Enter...`);
  const pos = await getCursor();
  layout[key] = { x: pos.x, y: pos.y };
  console.log(`    ✓ ${key}: { x: ${pos.x}, y: ${pos.y} }`);
}

// Also capture a screenshot for reference
await gui({ action: 'screenshot', path: 'demo-output/calibration-reference.png' });
console.log('\n  Screenshot saved: demo-output/calibration-reference.png');

// Output the layout object
console.log('\n=== Copy this into demo-acts.mjs (replace the L object) ===\n');
console.log('const L = {');
for (const [key, val] of Object.entries(layout)) {
  console.log(`  ${key}: { x: ${val.x}, y: ${val.y} },`);
}

// Derive tabBar.y from tab1
if (layout.tab1) {
  console.log(`  tabBar: { y: ${layout.tab1.y} },`);
}

console.log('};');
console.log('');

rl.close();
py.stdin.end();
py.kill();
process.exit(0);
