#!/usr/bin/env node
// Diagnostic: empirically determine the correct coordinate mapping.
// Moves the mouse to where we THINK the "New Workspace" button is,
// trying different scaling strategies. Watch where the cursor lands.
//
// Usage: node scripts/test-coordinates.mjs
// (Godly Terminal must be running + MCP server on port 8089)

import { execSync, spawn } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';
import { createInterface } from 'readline';
import { McpClient } from './mcp-client.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));

// ─── Find Python ───
function findPython() {
  for (const cmd of ['python', 'python3', 'python3.13']) {
    try {
      execSync(`${cmd} -c "import pyautogui"`, { stdio: 'pipe', timeout: 5000 });
      return cmd;
    } catch {}
  }
  const storePy = 'C:/Users/alanm/AppData/Local/Microsoft/WindowsApps/PythonSoftwareFoundation.Python.3.13_qbz5n2kfra8p0/python.exe';
  try {
    execSync(`"${storePy}" -c "import pyautogui"`, { stdio: 'pipe', timeout: 5000 });
    return storePy;
  } catch {}
  return 'python';
}

// ─── GUI Bridge (minimal) ───
let guiBridge = null;
let guiResolve = null;

async function startBridge() {
  const pythonCmd = findPython();
  console.log(`Python: ${pythonCmd}`);
  return new Promise((resolve, reject) => {
    const py = spawn(pythonCmd, [path.join(__dirname, 'gui-bridge.py')], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    py.stderr.on('data', d => { const m = d.toString().trim(); if (m) console.log(`  [bridge] ${m}`); });
    py.on('error', e => reject(e));
    const rl = createInterface({ input: py.stdout });
    rl.on('line', line => {
      try {
        const data = JSON.parse(line);
        if (data.ready) { guiBridge = py; resolve(); return; }
        if (guiResolve) { const cb = guiResolve; guiResolve = null; cb(data); }
      } catch {}
    });
    setTimeout(() => reject(new Error('timeout')), 10000);
  });
}

function gui(cmd) {
  return new Promise((resolve, reject) => {
    const t = setTimeout(() => { guiResolve = null; reject(new Error('timeout')); }, 10000);
    guiResolve = r => { clearTimeout(t); resolve(r); };
    guiBridge.stdin.write(JSON.stringify(cmd) + '\n');
  });
}

function sleep(ms) { return new Promise(r => setTimeout(r, ms)); }

// ─── Main ───
console.log('=== Coordinate Mapping Diagnostic ===\n');

// 1. Start bridge + get pyautogui screen size
await startBridge();
const ping = await gui({ action: 'ping' });
console.log(`pyautogui screen: ${ping.screenWidth}x${ping.screenHeight}`);

// 2. Get cursor position for reference
const cursor = await gui({ action: 'cursor' });
console.log(`Current cursor: (${cursor.x}, ${cursor.y}) [pyautogui coords]`);

// 3. Query webview layout
const client = new McpClient(8089);
await client.connect();
console.log(`MCP connected\n`);

const jsResult = await client.callTool('execute_js', { script: `
  function rect(el) {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: Math.round(r.x + r.width/2), y: Math.round(r.y + r.height/2), w: Math.round(r.width), h: Math.round(r.height) };
  }
  const addWsBtn = rect(document.querySelector('.add-workspace-btn'));
  const firstWs = rect(document.querySelector('.workspace-item'));
  return {
    addWsBtn, firstWs,
    screenX: window.screenX, screenY: window.screenY,
    screenW: window.screen.width, screenH: window.screen.height,
    dpr: window.devicePixelRatio,
    innerW: window.innerWidth, innerH: window.innerHeight,
  };
` });

const text = jsResult.content[0].text;
const data = JSON.parse(JSON.parse(text).result);
console.log('Webview reports:');
console.log(`  screen: ${data.screenW}x${data.screenH} (CSS)`);
console.log(`  screenX=${data.screenX}, screenY=${data.screenY} (CSS)`);
console.log(`  devicePixelRatio: ${data.dpr}`);
console.log(`  innerSize: ${data.innerW}x${data.innerH}`);
console.log(`  addWsBtn: (${data.addWsBtn?.x}, ${data.addWsBtn?.y}) viewport CSS`);
console.log(`  firstWs:  (${data.firstWs?.x}, ${data.firstWs?.y}) viewport CSS`);

// 4. Compute different coordinate strategies
const btn = data.addWsBtn;
if (!btn) { console.log('\nERROR: addWsBtn not found'); process.exit(1); }

const strategies = [
  {
    name: 'A: No scaling (CSS coords + offset)',
    x: btn.x + data.screenX,
    y: btn.y + data.screenY,
  },
  {
    name: 'B: Scale by devicePixelRatio',
    x: Math.round((btn.x + data.screenX) * data.dpr),
    y: Math.round((btn.y + data.screenY) * data.dpr),
  },
  {
    name: 'C: Scale by pyautogui/webview ratio',
    x: Math.round((btn.x + data.screenX) * (ping.screenWidth / data.screenW)),
    y: Math.round((btn.y + data.screenY) * (ping.screenHeight / data.screenH)),
  },
  {
    name: 'D: Scale offset separately (physical offset + CSS element * dpr)',
    x: Math.round(data.screenX * data.dpr + btn.x * data.dpr),
    y: Math.round(data.screenY * data.dpr + btn.y * data.dpr),
  },
  {
    name: 'E: No offset scaling, only element scaling',
    x: Math.round(data.screenX + btn.x * data.dpr),
    y: Math.round(data.screenY + btn.y * data.dpr),
  },
];

console.log('\n--- Testing strategies (watch the cursor!) ---');
console.log('Target: "+ New Workspace" button at bottom of sidebar\n');

for (const s of strategies) {
  console.log(`${s.name}: (${s.x}, ${s.y})`);
  await gui({ action: 'move', x: s.x, y: s.y, duration: 0.5 });
  await sleep(2000); // pause so user can see
  const pos = await gui({ action: 'cursor' });
  console.log(`  → cursor landed at: (${pos.x}, ${pos.y})\n`);
}

console.log('Done! Which strategy landed on the button?');

await client.close();
guiBridge.stdin.end();
guiBridge.kill();
process.exit(0);
