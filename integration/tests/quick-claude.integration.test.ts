/**
 * Quick Claude flow tests — the real end-to-end flow.
 *
 * Mirrors the exact sequence from commands/terminal.rs quick_claude_background():
 *   1. Create session in fresh temp directory
 *   2. Wait for shell ready (idle 500ms)
 *   3. Write `claude` command
 *   4. Wait 5s for Claude startup
 *   5. Detect trust prompt ("Do you trust the files" / "I trust this folder")
 *   6. Send Enter to accept
 *   7. Wait for Claude to become idle (400ms threshold)
 *   8. Write prompt text (without Enter)
 *   9. Poll SearchBuffer until prompt text is echoed
 *   10. Send Enter as separate write
 *   11. Verify Claude processes the prompt
 *
 * Prerequisites:
 *   - `claude` must be on PATH
 *   - daemon binary must be built: `npm run build:daemon`
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { execSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { DaemonFixture } from '../daemon-fixture.js';
import { DaemonClient } from '../daemon-client.js';
import { SessionHandle } from '../session-handle.js';

// ── Dependency check ─────────────────────────────────────────────────────

function isClaudeAvailable(): boolean {
  try {
    execSync('claude --version', { stdio: 'pipe', timeout: 10_000 });
    return true;
  } catch {
    return false;
  }
}

const HAS_CLAUDE = isClaudeAvailable();

// ── Primitive tests (no Claude dependency) ───────────────────────────────

describe('quick-claude: primitives', () => {
  let fixture: DaemonFixture;
  let client: DaemonClient;
  let session: SessionHandle;

  beforeAll(async () => {
    fixture = new DaemonFixture({ name: 'qc-prim' });
    await fixture.spawn();
    client = await fixture.connect();
    session = await SessionHandle.create(client, {
      id: 'qc-prim',
      shellType: 'cmd',
    });
    await session.waitForIdle(500, { timeoutMs: 10_000 });
  }, 20_000);

  afterAll(async () => {
    try { await session.close(); } catch { /* */ }
    client.disconnect();
    await fixture.teardown();
  }, 10_000);

  it('should detect shell ready via idle timeout', async () => {
    const resp = await client.sendRequest({
      type: 'GetLastOutputTime',
      session_id: session.sessionId,
    });
    expect(resp.type).toBe('LastOutputTime');
    if (resp.type === 'LastOutputTime') {
      expect(resp.running).toBe(true);
      expect(resp.epoch_ms).toBeGreaterThan(0);
    }
  }, 15_000);

  it('should write command and detect output', async () => {
    await session.writeCommand('echo QC_MARKER_123');
    await session.waitForText('QC_MARKER_123', { timeoutMs: 10_000 });

    const result = await session.searchBuffer('QC_MARKER_123');
    expect(result.found).toBe(true);
    expect(result.running).toBe(true);
  }, 15_000);

  it('should write text then enter separately (ink-safe pattern)', async () => {
    await session.writeTextThenEnter('echo INK_SAFE_TEST', 200);
    await session.waitForText('INK_SAFE_TEST', { timeoutMs: 10_000 });

    const result = await session.searchBuffer('INK_SAFE_TEST');
    expect(result.found).toBe(true);
  }, 15_000);

  it('should read grid after command', async () => {
    await session.writeCommand('echo GRID_SNAP');
    await session.waitForText('GRID_SNAP', { timeoutMs: 10_000 });
    await session.waitForIdle(300, { timeoutMs: 5_000 });

    const grid = await session.readGrid();
    expect(grid.cols).toBe(80);
    expect(grid.num_rows).toBe(24);
    expect(grid.rows.join('\n')).toContain('GRID_SNAP');
  }, 15_000);

  it('should reflect resize in grid dimensions', async () => {
    await session.resize(30, 120);
    await session.waitForIdle(300, { timeoutMs: 5_000 });

    const grid = await session.readGrid();
    expect(grid.cols).toBe(120);
    expect(grid.num_rows).toBe(30);
  }, 10_000);
});

// ── End-to-end Quick Claude flow ─────────────────────────────────────────

describe('quick-claude: end-to-end', () => {
  let fixture: DaemonFixture;
  let client: DaemonClient;
  let session: SessionHandle;
  let tempDir: string;

  beforeAll(async () => {
    if (!HAS_CLAUDE) return;

    // Create a fresh temp directory (triggers trust prompt on first launch)
    tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'godly-qc-e2e-'));

    fixture = new DaemonFixture({ name: 'qc-e2e' });
    await fixture.spawn();
    client = await fixture.connect();
    session = await SessionHandle.create(client, {
      id: 'qc-e2e',
      shellType: 'cmd',
      rows: 40,
      cols: 120,
      cwd: tempDir,
    });
  }, 30_000);

  afterAll(async () => {
    if (!HAS_CLAUDE) return;

    try { await session.close(); } catch { /* */ }
    client?.disconnect();
    await fixture?.teardown();

    // Clean up temp directory
    try { fs.rmSync(tempDir, { recursive: true, force: true }); } catch { /* */ }
  }, 15_000);

  it('should have claude available on PATH', () => {
    expect(HAS_CLAUDE).toBe(true);
  });

  it('should launch claude in a new directory and handle trust prompt', async () => {
    if (!HAS_CLAUDE) return;

    // Step 1: Wait for shell ready (matches quick_claude_background: 500ms idle, 5s timeout)
    await session.waitForIdle(500, { timeoutMs: 10_000 });

    // Step 2: Write claude command (matches the exact command from terminal.rs)
    await session.writeCommand('claude --dangerously-skip-permissions');

    // Step 3: Wait for Claude to start producing output (5s like the real flow)
    // Claude outputs version banner, config loading, etc.
    await sleep(5_000);

    // Step 4: Check for trust prompt
    // The real flow searches for both "Do you trust the files" and "I trust this folder"
    const trustNeedles = ['Do you trust the files', 'I trust this folder'];
    let foundTrustPrompt = false;

    for (const needle of trustNeedles) {
      const result = await session.searchBuffer(needle, true);
      if (result.found) {
        foundTrustPrompt = true;
        break;
      }
    }

    // Step 5: If trust prompt found, send Enter to accept (option 1 is pre-selected)
    if (foundTrustPrompt) {
      await session.write('\r');
      // Give Claude time to process acceptance (3s like the real flow)
      await sleep(3_000);
    }

    // Step 6: Wait for Claude to become idle
    // Claude's ink TUI blinks cursor every ~500ms; use 400ms threshold like the real flow
    // 25s timeout (30s total with the 5s above)
    await session.waitForIdle(400, { timeoutMs: 25_000, pollMs: 100 });

    // Step 7: Small delay before writing prompt (300ms like the real flow)
    await sleep(300);

    // Step 8: Write prompt text WITHOUT Enter
    const prompt = 'say exactly INTEGRATION_TEST_OK and nothing else';
    await session.write(prompt);

    // Step 9: Poll SearchBuffer until prompt text is echoed by ink TUI
    // (first 40 chars, strip_ansi, 30s timeout like the real flow)
    const searchPrefix = prompt.slice(0, 40);
    await session.waitForText(searchPrefix, { timeoutMs: 30_000, pollMs: 250, stripAnsi: true });

    // Step 10: Small buffer after echo detection (200ms like the real flow), then Enter
    await sleep(200);
    await session.write('\r');

    // Step 11: Verify Claude processes the prompt
    // Wait for Claude's response to appear in the buffer
    await session.waitForText('INTEGRATION_TEST_OK', { timeoutMs: 60_000, pollMs: 500, stripAnsi: true });

    const result = await session.searchBuffer('INTEGRATION_TEST_OK', true);
    expect(result.found).toBe(true);
  }, 120_000); // 2 minute timeout for full flow
});

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
