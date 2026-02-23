#!/usr/bin/env node
// scripts/record-demo.mjs
// Fully automatic demo recording system for Godly Terminal.
//
// Usage:
//   node scripts/record-demo.mjs              # Full recording
//   node scripts/record-demo.mjs --dry-run    # Execute demo without recording
//   node scripts/record-demo.mjs --phone-only # Only run phone (Playwright) acts
//
// Prerequisites:
//   - Godly Terminal + daemon running
//   - godly-mcp SSE server (auto-started via --ensure)
//   - godly-remote running on port 3377 (for phone demo)
//   - npm install (installs playwright, @ffmpeg-installer/ffmpeg, fluent-ffmpeg, eventsource)

import { execSync, spawn } from 'child_process';
import { existsSync, mkdirSync, rmSync } from 'fs';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const OUTPUT_DIR = path.join(ROOT, 'demo-output');
const SCREENSHOTS_DIR = path.join(OUTPUT_DIR, 'screenshots');

// ─── CLI Args ───
const args = process.argv.slice(2);
const DRY_RUN = args.includes('--dry-run');
const PHONE_ONLY = args.includes('--phone-only');
const MCP_PORT = parseInt(args.find(a => a.startsWith('--mcp-port='))?.split('=')[1] || '8089', 10);
const PHONE_URL = args.find(a => a.startsWith('--phone-url='))?.split('=')[1] || 'http://localhost:3377/phone';

// ─── Helpers ───
function log(msg) {
  const ts = new Date().toISOString().slice(11, 19);
  console.log(`[${ts}] ${msg}`);
}

function sleep(ms) {
  return new Promise(r => setTimeout(r, ms));
}

/** Resolve $var.field references in args against context */
function resolveArgs(obj, context) {
  if (typeof obj === 'string') {
    if (obj.startsWith('$')) {
      const ref = obj.slice(1); // e.g. "backendWs.workspace_id"
      const parts = ref.split('.');
      let val = context;
      for (const p of parts) {
        if (val == null) return obj;
        val = val[p];
      }
      return val ?? obj;
    }
    return obj;
  }
  if (Array.isArray(obj)) {
    return obj.map(item => resolveArgs(item, context));
  }
  if (obj && typeof obj === 'object') {
    const result = {};
    for (const [k, v] of Object.entries(obj)) {
      result[k] = resolveArgs(v, context);
    }
    return result;
  }
  return obj;
}

/** Parse MCP tool result content — extract text or JSON from content array */
function parseToolResult(result) {
  if (!result) return {};
  // MCP tools/call returns { content: [{ type: "text", text: "..." }] }
  const content = result.content;
  if (Array.isArray(content)) {
    for (const item of content) {
      if (item.type === 'text' && item.text) {
        try {
          return JSON.parse(item.text);
        } catch {
          return { text: item.text };
        }
      }
    }
  }
  return result;
}

// ─── Phase 0: Pre-flight ───
async function preflight() {
  log('Phase 0: Pre-flight checks');

  mkdirSync(OUTPUT_DIR, { recursive: true });
  mkdirSync(SCREENSHOTS_DIR, { recursive: true });

  // Check godly-mcp binary exists
  const mcpExe = path.join(ROOT, 'src-tauri/target/release/godly-mcp.exe');
  const mcpDebug = path.join(ROOT, 'src-tauri/target/debug/godly-mcp.exe');
  const mcpBin = existsSync(mcpExe) ? mcpExe : existsSync(mcpDebug) ? mcpDebug : null;
  if (!mcpBin) {
    console.error('ERROR: godly-mcp.exe not found. Run: npm run build:mcp');
    process.exit(1);
  }

  // Ensure MCP SSE server is running
  log(`Ensuring MCP SSE server on port ${MCP_PORT}...`);
  try {
    execSync(`"${mcpBin}" --ensure ${MCP_PORT}`, { stdio: 'pipe', timeout: 10000 });
    log('MCP SSE server ready');
  } catch (e) {
    log(`Warning: --ensure failed (${e.message}), server may already be running`);
  }

  // Verify MCP SSE server is reachable
  try {
    const resp = await fetch(`http://127.0.0.1:${MCP_PORT}/sse`, {
      signal: AbortSignal.timeout(3000),
    });
    if (resp.ok || resp.status === 200) {
      log('MCP SSE server reachable');
    }
  } catch {
    // SSE endpoint may not respond to a plain fetch — that's fine, we'll connect properly later
    log('MCP SSE endpoint contacted (SSE will connect in Phase 2)');
  }
}

// ─── Phase 1: Start Recording ───
let ffmpegProcess = null;
let playwrightBrowser = null;
let playwrightContext = null;
let playwrightPage = null;

async function startRecording() {
  if (DRY_RUN) {
    log('Phase 1: DRY RUN — skipping recording setup');
    return;
  }
  log('Phase 1: Starting recording');

  // FFmpeg: capture full desktop
  const ffmpegPath = await getFFmpegPath();
  const rawDesktop = path.join(OUTPUT_DIR, 'raw-desktop.mp4');

  // Remove stale output
  if (existsSync(rawDesktop)) rmSync(rawDesktop);

  log('Starting FFmpeg desktop capture...');
  ffmpegProcess = spawn(ffmpegPath, [
    '-f', 'gdigrab',
    '-framerate', '30',
    '-i', 'desktop',
    '-c:v', 'libx264',
    '-preset', 'ultrafast',
    '-crf', '18',
    '-pix_fmt', 'yuv420p',
    rawDesktop,
  ], {
    stdio: ['pipe', 'pipe', 'pipe'],
  });

  ffmpegProcess.stderr.on('data', (data) => {
    const str = data.toString();
    if (str.includes('Error') || str.includes('error')) {
      log(`FFmpeg: ${str.trim()}`);
    }
  });

  // Give FFmpeg a moment to start
  await sleep(1500);
  log('FFmpeg desktop capture started');

  // Playwright: phone browser with video recording
  await startPlaywright();
}

async function getFFmpegPath() {
  try {
    const { default: ffmpegInstaller } = await import('@ffmpeg-installer/ffmpeg');
    return ffmpegInstaller.path;
  } catch {
    // Fallback: check if ffmpeg is on PATH
    try {
      execSync('ffmpeg -version', { stdio: 'pipe' });
      return 'ffmpeg';
    } catch {
      console.error('ERROR: FFmpeg not found. Install: npm install @ffmpeg-installer/ffmpeg');
      process.exit(1);
    }
  }
}

async function startPlaywright() {
  log('Starting Playwright phone browser...');
  const { chromium } = await import('playwright');

  playwrightBrowser = await chromium.launch({
    headless: false,
    args: [
      '--window-size=430,932',
      '--window-position=1400,100', // Right side of screen
    ],
  });

  playwrightContext = await playwrightBrowser.newContext({
    viewport: { width: 393, height: 852 },
    deviceScaleFactor: 3,
    isMobile: true,
    hasTouch: true,
    userAgent: 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1',
    recordVideo: {
      dir: OUTPUT_DIR,
      size: { width: 393, height: 852 },
    },
  });

  playwrightPage = await playwrightContext.newPage();
  log('Playwright phone browser ready');
}

// ─── Phase 2: Execute Demo ───
async function executeDemoActs(mcpClient) {
  const { acts } = await import('./demo-acts.mjs');
  const context = {}; // Stores results from storeAs

  const actsToRun = PHONE_ONLY
    ? acts.filter(a => a.id === 'phone-remote')
    : acts;

  for (const act of actsToRun) {
    log(`\n${'='.repeat(50)}`);
    log(`ACT: ${act.caption}`);
    log('='.repeat(50));

    for (const step of act.steps) {
      try {
        await executeStep(step, context, mcpClient);
        if (step.delay) await sleep(step.delay);
      } catch (err) {
        log(`  WARNING: Step failed (${step.type}/${step.tool || step.action || ''}): ${err.message}`);
        // Continue with next step — demo should be resilient
      }
    }
  }

  return context;
}

async function executeStep(step, context, mcpClient) {
  switch (step.type) {
    case 'mcp': {
      const resolvedArgs = resolveArgs(step.args, context);
      log(`  MCP: ${step.tool}(${JSON.stringify(resolvedArgs).slice(0, 80)}...)`);
      const result = await mcpClient.callTool(step.tool, resolvedArgs);
      if (step.storeAs) {
        context[step.storeAs] = parseToolResult(result);
        log(`    -> stored as ${step.storeAs}: ${JSON.stringify(context[step.storeAs]).slice(0, 100)}`);
      }
      break;
    }

    case 'playwright': {
      if (!playwrightPage && !DRY_RUN) {
        log('  Playwright not available, skipping');
        break;
      }
      if (DRY_RUN) {
        log(`  Playwright (dry): ${step.action}`);
        break;
      }
      await executePlaywrightStep(step);
      break;
    }

    case 'pause': {
      log(`  Pause ${step.ms}ms`);
      await sleep(step.ms);
      break;
    }

    case 'log': {
      log(`  ${step.message}`);
      break;
    }

    case 'cleanup-terminals': {
      await cleanupTerminals(context, mcpClient);
      break;
    }

    default:
      log(`  Unknown step type: ${step.type}`);
  }
}

async function executePlaywrightStep(step) {
  const page = playwrightPage;
  switch (step.action) {
    case 'goto':
      log(`  Playwright: goto ${step.url}`);
      await page.goto(step.url, { waitUntil: 'domcontentloaded', timeout: 15000 });
      break;

    case 'screenshot': {
      const filePath = path.join(SCREENSHOTS_DIR, `${step.name}.png`);
      await page.screenshot({ path: filePath });
      log(`  Playwright: screenshot -> ${step.name}.png`);
      break;
    }

    case 'fill-if-visible': {
      const el = page.locator(step.selector).first();
      if (await el.isVisible({ timeout: 2000 }).catch(() => false)) {
        if (step.value) {
          await el.fill(step.value);
          log(`  Playwright: filled ${step.selector}`);
        } else {
          await el.press('Enter');
          log(`  Playwright: pressed Enter on ${step.selector}`);
        }
      } else {
        log(`  Playwright: ${step.selector} not visible, skipping`);
      }
      break;
    }

    case 'tap-if-visible': {
      const el = page.locator(step.selector).first();
      if (await el.isVisible({ timeout: 2000 }).catch(() => false)) {
        await el.tap();
        log(`  Playwright: tapped ${step.selector}`);
      } else {
        log(`  Playwright: ${step.selector} not visible, skipping`);
      }
      break;
    }

    case 'scroll':
      await page.evaluate((y) => window.scrollBy(0, y), step.y || 200);
      log(`  Playwright: scrolled by ${step.y}px`);
      break;

    default:
      log(`  Playwright: unknown action ${step.action}`);
  }
}

async function cleanupTerminals(context, mcpClient) {
  // Close all terminals we created during the demo
  const terminalKeys = Object.keys(context).filter(k =>
    context[k]?.terminal_id
  );
  for (const key of terminalKeys) {
    try {
      await mcpClient.callTool('close_terminal', { terminal_id: context[key].terminal_id });
      log(`    Closed terminal: ${key} (${context[key].terminal_id})`);
      await sleep(300);
    } catch {
      // Terminal may already be closed
    }
  }
}

// ─── Phase 3: Stop Recording ───
async function stopRecording() {
  if (DRY_RUN) {
    log('Phase 3: DRY RUN — nothing to stop');
    return;
  }
  log('Phase 3: Stopping recording');

  // Stop FFmpeg gracefully with 'q'
  if (ffmpegProcess) {
    log('Stopping FFmpeg...');
    ffmpegProcess.stdin.write('q');
    await new Promise((resolve) => {
      ffmpegProcess.on('close', resolve);
      // Force kill after 10s
      setTimeout(() => {
        ffmpegProcess.kill('SIGKILL');
        resolve();
      }, 10000);
    });
    log('FFmpeg stopped');
  }

  // Close Playwright context (finalizes video)
  if (playwrightContext) {
    log('Closing Playwright...');
    const videoPath = await playwrightPage?.video()?.path();
    await playwrightContext.close();
    await playwrightBrowser?.close();
    if (videoPath) {
      log(`Playwright video saved: ${videoPath}`);
    }
    log('Playwright closed');
  }
}

// ─── Phase 4: Post-Processing ───
async function postProcess() {
  if (DRY_RUN) {
    log('Phase 4: DRY RUN — skipping post-processing');
    return;
  }
  log('Phase 4: Post-processing');

  const ffmpegPath = await getFFmpegPath();
  const rawDesktop = path.join(OUTPUT_DIR, 'raw-desktop.mp4');
  const finalOutput = path.join(OUTPUT_DIR, 'godly-terminal-demo.mp4');
  const captionsFile = path.join(__dirname, 'captions.srt');

  if (!existsSync(rawDesktop)) {
    log('WARNING: raw-desktop.mp4 not found, skipping post-processing');
    return;
  }

  // Find the Playwright phone video (it's auto-named)
  const { readdirSync } = await import('fs');
  const phoneVideos = readdirSync(OUTPUT_DIR).filter(f =>
    f.endsWith('.webm') && f !== 'raw-desktop.mp4'
  );
  const phoneVideo = phoneVideos.length > 0
    ? path.join(OUTPUT_DIR, phoneVideos[0])
    : null;

  if (phoneVideo) {
    log('Compositing desktop + phone side-by-side...');
    // Side-by-side composite with captions
    const filterComplex = [
      // Scale desktop to 1300x1080 (crop center if needed)
      '[0:v]scale=1300:1080:force_original_aspect_ratio=decrease,pad=1300:1080:(ow-iw)/2:(oh-ih)/2[desktop]',
      // Scale phone to match height
      '[1:v]scale=-1:1080[phone]',
      // Stack horizontally
      '[desktop][phone]hstack=inputs=2[composite]',
    ];

    // Add captions if SRT file exists
    if (existsSync(captionsFile)) {
      filterComplex.push(
        `[composite]subtitles='${captionsFile.replace(/\\/g, '/').replace(/:/g, '\\\\:')}'[final]`
      );
    } else {
      filterComplex.push('[composite]copy[final]');
    }

    const ffArgs = [
      '-i', rawDesktop,
      '-i', phoneVideo,
      '-filter_complex', filterComplex.join(';'),
      '-map', '[final]',
      '-c:v', 'libx264',
      '-preset', 'medium',
      '-crf', '20',
      '-pix_fmt', 'yuv420p',
      '-y',
      finalOutput,
    ];

    log(`FFmpeg composite: ${ffArgs.join(' ').slice(0, 120)}...`);
    execSync(`"${ffmpegPath}" ${ffArgs.map(a => `"${a}"`).join(' ')}`, {
      stdio: 'inherit',
      timeout: 300000, // 5 min
    });
  } else {
    log('No phone video found, using desktop only with captions...');
    const ffArgs = ['-i', rawDesktop];

    if (existsSync(captionsFile)) {
      ffArgs.push(
        '-vf', `subtitles='${captionsFile.replace(/\\/g, '/').replace(/:/g, '\\\\:')}'`,
      );
    }

    ffArgs.push(
      '-c:v', 'libx264',
      '-preset', 'medium',
      '-crf', '20',
      '-pix_fmt', 'yuv420p',
      '-y',
      finalOutput,
    );

    execSync(`"${ffmpegPath}" ${ffArgs.map(a => `"${a}"`).join(' ')}`, {
      stdio: 'inherit',
      timeout: 300000,
    });
  }

  log(`Final output: ${finalOutput}`);

  // Clean up raw files
  log('Cleaning up raw recordings...');
  // Keep raw files for debugging — user can delete manually
}

// ─── Main ───
async function main() {
  log('Godly Terminal Demo Recorder');
  log(`Mode: ${DRY_RUN ? 'DRY RUN' : PHONE_ONLY ? 'PHONE ONLY' : 'FULL RECORDING'}`);
  log('');

  // Phase 0: Pre-flight
  await preflight();

  // Phase 1: Start recording
  await startRecording();

  // Phase 2: Connect MCP and execute demo
  log('\nPhase 2: Executing demo sequence');
  const { McpClient } = await import('./mcp-client.mjs');
  const mcpClient = new McpClient(MCP_PORT);

  try {
    log('Connecting to MCP SSE server...');
    await mcpClient.connect();
    log(`MCP connected (session: ${mcpClient.sessionId})`);

    // Small delay for recording to capture the clean initial state
    if (!DRY_RUN) await sleep(2000);

    await executeDemoActs(mcpClient);
  } finally {
    await mcpClient.close();
  }

  // Phase 3: Stop recording
  await stopRecording();

  // Phase 4: Post-process
  await postProcess();

  log('\nDone! Check demo-output/ for results.');
  process.exit(0);
}

// Handle cleanup on unexpected exit
process.on('SIGINT', async () => {
  log('\nInterrupted — cleaning up...');
  await stopRecording();
  process.exit(1);
});

process.on('unhandledRejection', (err) => {
  log(`Unhandled error: ${err.message}`);
  console.error(err);
});

main().catch(async (err) => {
  log(`Fatal error: ${err.message}`);
  console.error(err);
  await stopRecording();
  process.exit(1);
});
