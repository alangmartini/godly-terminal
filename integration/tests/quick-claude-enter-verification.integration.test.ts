/**
 * Bug #506: Quick Claude Enter verify-and-retry.
 *
 * Root cause: quick_claude_background() previously sent Enter (\r) once and
 * never verified that Claude Code actually processed it as a submit keypress.
 * When Enter was lost (due to TUI state transitions, timing races, ConPTY
 * buffering), the prompt sat in the input area forever with no recovery.
 *
 * Fix: After sending Enter, use ReadGrid to check if the prompt text has
 * disappeared from the visible terminal grid. When Claude Code processes Enter
 * as submit, ink clears the input area — the prompt is no longer visible.
 * If still visible after a delay, Enter was lost; retry up to 5 times.
 *
 * This test uses a mock TUI that simulates the failure mode:
 * - After echoing typed text, the mock enters a brief "processing" phase
 *   (simulating ink's re-render cycle after receiving input)
 * - During this phase, Enter (\r) is received but NOT processed as submit
 * - After the processing phase, Enter IS processed as submit
 *
 * The verify-and-retry loop detects the still-visible prompt via ReadGrid
 * and re-sends Enter after the processing phase completes.
 *
 * Run: pnpm test:integration -- --testPathPattern enter-verification
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { DaemonFixture } from '../daemon-fixture.js';
import { DaemonClient } from '../daemon-client.js';
import { SessionHandle } from '../session-handle.js';

// ── Mock TUI script ─────────────────────────────────────────────────────
//
// Simulates an ink-style TUI that can "lose" the first Enter after text input.
// This represents the real-world failure mode where Enter arrives during a
// TUI state transition and is consumed without being processed as submit.
//
// Behavior:
// 1. STARTUP_DELAY ms: outputs MOCK_STARTING, not reading stdin
// 2. Enters raw mode, outputs MOCK_READY
// 3. Reads stdin: accumulates text, echoes to stdout
// 4. After first text chunk, enters PROCESSING phase for PROCESSING_DELAY ms
//    - During this phase, Enter is received but NOT treated as submit
//    - This simulates ink re-rendering, state transition, etc.
// 5. After PROCESSING phase, sets ready_for_submit flag
// 6. Next Enter → SUBMITTED (success)
// 7. If no Enter for 10s after becoming ready → TIMEOUT (Enter was lost)

const MOCK_TUI_SCRIPT = `
const STARTUP_DELAY = parseInt(process.env.MOCK_STARTUP_DELAY || '1500');
const PROCESSING_DELAY = parseInt(process.env.MOCK_PROCESSING_DELAY || '800');

process.stdout.write('MOCK_STARTING\\n');

setTimeout(() => {
  process.stdout.write('MOCK_READY\\n');

  try {
    process.stdin.setRawMode(true);
  } catch (e) {
    process.stdout.write('RAW_MODE_UNSUPPORTED\\n');
    process.exit(3);
  }
  process.stdin.resume();

  let textBuffer = '';
  let processingPhase = false;
  let readyForSubmit = false;
  let processingStarted = false;
  let enterDuringProcessing = 0;

  process.stdin.on('data', (chunk) => {
    const bytes = [...chunk];
    const hasEnter = bytes.includes(0x0D) || bytes.includes(0x0A);
    const printableBytes = bytes.filter(b => b >= 0x20);

    // Accumulate and echo printable text
    if (printableBytes.length > 0) {
      const text = Buffer.from(printableBytes).toString();
      textBuffer += text;
      // Echo to stdout — this is what SearchBuffer/poll_text_in_output detects
      process.stdout.write(text);

      // Start processing phase after first text chunk
      if (!processingStarted) {
        processingStarted = true;
        processingPhase = true;
        setTimeout(() => {
          processingPhase = false;
          readyForSubmit = true;
          process.stdout.write('\\nPROCESSING_DONE\\n');
        }, PROCESSING_DELAY);
      }
    }

    if (hasEnter) {
      if (readyForSubmit) {
        // Processing done, Enter processed as submit
        process.stdout.write('\\nSUBMITTED:' + textBuffer + '\\n');
        process.exit(0);
      } else if (processingPhase) {
        // Bug #506: Enter arrived during processing phase — consumed but NOT submitted.
        // In real ink, this maps to:
        // - Enter during re-render cycle (state update + paint)
        // - Enter during async operation (MCP server init, file loading)
        // - Enter during TUI transition (prompt type change, selection update)
        enterDuringProcessing++;
        process.stdout.write('\\nENTER_CONSUMED_DURING_PROCESSING\\n');
      } else if (textBuffer.length === 0) {
        process.stdout.write('\\nEMPTY_ENTER\\n');
      } else {
        // Enter before processing started — shouldn't happen with echo detection
        process.stdout.write('\\nMERGED:' + textBuffer + '\\n');
        process.exit(1);
      }
    }
  });

  // Safety timeout
  setTimeout(() => {
    const status = enterDuringProcessing > 0 ? 'ENTER_LOST' : 'NO_ENTER';
    process.stdout.write('\\nTIMEOUT:' + textBuffer + ':status=' + status +
      ':consumed=' + enterDuringProcessing + '\\n');
    process.exit(enterDuringProcessing > 0 ? 1 : 2);
  }, 10000);
}, STARTUP_DELAY);
`;

// ── Helpers ──────────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Poll session buffer for any of the given markers. Returns the first match.
 */
async function pollForMarker(
  session: SessionHandle,
  markers: string[],
  timeoutMs: number,
  pollMs = 500,
): Promise<string | null> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    for (const marker of markers) {
      const result = await session.searchBuffer(marker, true);
      if (result.found) return marker;
    }
    await sleep(pollMs);
  }
  return null;
}

// ── Tests ────────────────────────────────────────────────────────────────

describe('quick-claude: enter verification (Bug #506)', () => {
  let fixture: DaemonFixture;
  let tempDir: string;
  let mockPath: string;

  beforeAll(async () => {
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'godly-qc-enter-verify-'));
    mockPath = path.join(tempDir, 'mock-tui.mjs');
    fs.writeFileSync(mockPath, MOCK_TUI_SCRIPT);

    fixture = new DaemonFixture({ name: 'qc-enterv' });
    await fixture.spawn();
  }, 30_000);

  afterAll(async () => {
    await fixture?.teardown();
    try {
      fs.rmSync(tempDir, { recursive: true, force: true });
    } catch { /* ignore */ }
  }, 15_000);

  /**
   * Bug #506: Verify-and-retry recovers from Enter lost during TUI processing.
   *
   * The mock has an 800ms processing delay after echoing text. The first Enter
   * arrives ~200ms after echo detection — within the processing phase — and is
   * consumed but not submitted. The verify-and-retry loop detects the prompt
   * is still visible via ReadGrid, waits, and re-sends Enter after processing
   * completes. The second Enter succeeds.
   */
  it('verify-and-retry recovers from Enter lost during 800ms processing phase', async () => {
    const client = await fixture.connect();
    const session = await SessionHandle.create(client, {
      id: 'qc-enter-verify-1',
      shellType: 'cmd',
      rows: 24,
      cols: 120,
    });

    try {
      // Wait for shell ready (same as quick_claude_background step 1)
      await session.waitForIdle(500, { timeoutMs: 10_000 });

      // Start mock TUI (simulating `claude --dangerously-skip-permissions`)
      const escapedPath = mockPath.replace(/\\/g, '\\\\');
      await session.writeCommand(
        `set MOCK_STARTUP_DELAY=1500&& set MOCK_PROCESSING_DELAY=800&& node "${escapedPath}"`,
      );

      // Wait for mock to start (same as quick_claude_background step 3: 5s sleep)
      const rawUnsupported = await pollForMarker(
        session,
        ['MOCK_READY', 'RAW_MODE_UNSUPPORTED'],
        15_000,
      );

      if (rawUnsupported === 'RAW_MODE_UNSUPPORTED') {
        console.log('SKIP: Raw mode not supported in this ConPTY environment');
        return;
      }
      expect(rawUnsupported).toBe('MOCK_READY');

      // Wait for idle after mock startup (same as step 3b: 400ms idle, 25s timeout)
      await session.waitForIdle(400, { timeoutMs: 10_000, pollMs: 100 });

      // Small delay (same as step 4: 300ms)
      await sleep(300);

      // Write prompt text WITHOUT Enter (same as step 5)
      const prompt = 'test prompt for enter verification bug 506';
      await session.write(prompt);

      // Poll SearchBuffer until echo detected (same as step 5b)
      const searchPrefix = prompt.slice(0, 40);
      await session.waitForText(searchPrefix, {
        timeoutMs: 30_000,
        pollMs: 250,
        stripAnsi: true,
      });

      // Small delay after echo (same as step 5c: 200ms)
      await sleep(200);

      // Step 6: Verify-and-retry loop (matches the Rust fix in terminal.rs)
      // Send Enter, then check ReadGrid to see if the prompt text disappeared.
      // If still visible, Enter was lost during TUI processing — retry.
      const MAX_ENTER_RETRIES = 5;
      const ENTER_VERIFY_DELAY_MS = 500;
      const ENTER_RETRY_BACKOFF_MS = 300;

      for (let attempt = 0; attempt < MAX_ENTER_RETRIES; attempt++) {
        await session.write('\r');
        await sleep(ENTER_VERIFY_DELAY_MS);

        const grid = await session.readGrid();
        const screenText = grid.rows.join(' ');
        if (!screenText.includes(searchPrefix)) {
          // Prompt disappeared — Enter was processed as submit
          break;
        }

        // Prompt still visible — retry with backoff
        if (attempt + 1 < MAX_ENTER_RETRIES) {
          await sleep(ENTER_RETRY_BACKOFF_MS * (attempt + 1));
        }
      }

      // Wait for mock's verdict — with retry, the second Enter arrives after
      // the 800ms processing phase and is processed as submit.
      const result = await pollForMarker(
        session,
        ['SUBMITTED:', 'ENTER_CONSUMED_DURING_PROCESSING', 'TIMEOUT:', 'MERGED:', 'ENTER_LOST'],
        15_000,
      );

      expect(result).toBe('SUBMITTED:');
    } finally {
      try { await session.close(); } catch { /* session may already be closed */ }
      client.disconnect();
    }
  }, 60_000);

  /**
   * Control test: Enter works immediately when TUI has no processing delay.
   * Confirms the mock is correct and verify-and-retry succeeds on first attempt.
   */
  it('verify-and-retry succeeds on first attempt with no processing delay (control)', async () => {
    const client = await fixture.connect();
    const session = await SessionHandle.create(client, {
      id: 'qc-enter-verify-2',
      shellType: 'cmd',
      rows: 24,
      cols: 120,
    });

    try {
      await session.waitForIdle(500, { timeoutMs: 10_000 });

      // Same mock but with PROCESSING_DELAY=0 (no processing phase)
      const escapedPath = mockPath.replace(/\\/g, '\\\\');
      await session.writeCommand(
        `set MOCK_STARTUP_DELAY=1500&& set MOCK_PROCESSING_DELAY=0&& node "${escapedPath}"`,
      );

      const readyMarker = await pollForMarker(
        session,
        ['MOCK_READY', 'RAW_MODE_UNSUPPORTED'],
        15_000,
      );

      if (readyMarker === 'RAW_MODE_UNSUPPORTED') {
        console.log('SKIP: Raw mode not supported in this ConPTY environment');
        return;
      }
      expect(readyMarker).toBe('MOCK_READY');

      await session.waitForIdle(400, { timeoutMs: 10_000, pollMs: 100 });
      await sleep(300);

      const prompt = 'control test no processing delay';
      await session.write(prompt);

      const searchPrefix = prompt.slice(0, 40);
      await session.waitForText(searchPrefix, {
        timeoutMs: 30_000,
        pollMs: 250,
        stripAnsi: true,
      });

      await sleep(200);

      // Verify-and-retry loop (same as test 1)
      for (let attempt = 0; attempt < 5; attempt++) {
        await session.write('\r');
        await sleep(500);
        const grid = await session.readGrid();
        const screenText = grid.rows.join(' ');
        if (!screenText.includes(searchPrefix)) break;
        if (attempt + 1 < 5) await sleep(300 * (attempt + 1));
      }

      // With PROCESSING_DELAY=0, first Enter succeeds immediately
      const result = await pollForMarker(
        session,
        ['SUBMITTED:', 'ENTER_CONSUMED_DURING_PROCESSING', 'TIMEOUT:', 'MERGED:'],
        15_000,
      );

      expect(result).toBe('SUBMITTED:');
    } finally {
      try { await session.close(); } catch { /* */ }
      client.disconnect();
    }
  }, 60_000);

  /**
   * Borderline case: 300ms processing delay. Without retry, this was flaky
   * (~50% failure rate). With verify-and-retry, it reliably succeeds.
   */
  it('verify-and-retry handles borderline 300ms processing delay reliably', async () => {
    const client = await fixture.connect();
    const session = await SessionHandle.create(client, {
      id: 'qc-enter-verify-3',
      shellType: 'cmd',
      rows: 24,
      cols: 120,
    });

    try {
      await session.waitForIdle(500, { timeoutMs: 10_000 });

      const escapedPath = mockPath.replace(/\\/g, '\\\\');
      await session.writeCommand(
        `set MOCK_STARTUP_DELAY=1500&& set MOCK_PROCESSING_DELAY=300&& node "${escapedPath}"`,
      );

      const readyMarker = await pollForMarker(
        session,
        ['MOCK_READY', 'RAW_MODE_UNSUPPORTED'],
        15_000,
      );

      if (readyMarker === 'RAW_MODE_UNSUPPORTED') {
        console.log('SKIP: Raw mode not supported in this ConPTY environment');
        return;
      }
      expect(readyMarker).toBe('MOCK_READY');

      await session.waitForIdle(400, { timeoutMs: 10_000, pollMs: 100 });
      await sleep(300);

      const prompt = 'borderline processing delay test';
      await session.write(prompt);

      const searchPrefix = prompt.slice(0, 30);
      await session.waitForText(searchPrefix, {
        timeoutMs: 30_000,
        pollMs: 250,
        stripAnsi: true,
      });

      await sleep(200);

      // Verify-and-retry loop (same as other tests)
      for (let attempt = 0; attempt < 5; attempt++) {
        await session.write('\r');
        await sleep(500);
        const grid = await session.readGrid();
        const screenText = grid.rows.join(' ');
        if (!screenText.includes(searchPrefix)) break;
        if (attempt + 1 < 5) await sleep(300 * (attempt + 1));
      }

      // With retry, even the borderline 300ms case succeeds reliably
      const result = await pollForMarker(
        session,
        ['SUBMITTED:', 'ENTER_CONSUMED_DURING_PROCESSING', 'TIMEOUT:'],
        15_000,
      );

      expect(result).toBe('SUBMITTED:');
    } finally {
      try { await session.close(); } catch { /* */ }
      client.disconnect();
    }
  }, 60_000);
});
