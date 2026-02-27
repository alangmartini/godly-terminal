/**
 * Quick Claude flow tests — validates the orchestration pattern from
 * commands/terminal.rs (idle detection, command write, search, grid read).
 *
 * These test the same patterns the app uses when launching Claude Code
 * in a terminal: detect idle, write command, wait for output, search buffer.
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { DaemonFixture } from '../daemon-fixture.js';
import { DaemonClient } from '../daemon-client.js';
import { SessionHandle } from '../session-handle.js';

describe('quick-claude: idle detection', () => {
  let fixture: DaemonFixture;
  let client: DaemonClient;
  let session: SessionHandle;

  beforeAll(async () => {
    fixture = new DaemonFixture({ name: 'qc-idle' });
    await fixture.spawn();
    client = await fixture.connect();
    session = await SessionHandle.create(client, {
      id: 'qc-idle',
      shellType: 'cmd',
    });
  }, 20_000);

  afterAll(async () => {
    try { await session.close(); } catch { /* */ }
    client.disconnect();
    await fixture.teardown();
  }, 10_000);

  it('should detect shell ready via idle timeout', async () => {
    // After cmd.exe starts, it should become idle within a few seconds
    await session.waitForIdle(500, { timeoutMs: 10_000 });

    // Verify we can check last output time
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
});

describe('quick-claude: command + output', () => {
  let fixture: DaemonFixture;
  let client: DaemonClient;
  let session: SessionHandle;

  beforeAll(async () => {
    fixture = new DaemonFixture({ name: 'qc-cmd' });
    await fixture.spawn();
    client = await fixture.connect();
    session = await SessionHandle.create(client, {
      id: 'qc-cmd',
      shellType: 'cmd',
    });
    await session.waitForIdle(500, { timeoutMs: 10_000 });
  }, 20_000);

  afterAll(async () => {
    try { await session.close(); } catch { /* */ }
    client.disconnect();
    await fixture.teardown();
  }, 10_000);

  it('should write command and detect output', async () => {
    await session.writeCommand('echo QC_MARKER_123');
    await session.waitForText('QC_MARKER_123', { timeoutMs: 10_000 });

    const result = await session.searchBuffer('QC_MARKER_123');
    expect(result.found).toBe(true);
    expect(result.running).toBe(true);
  }, 15_000);

  it('should write text then enter separately (ink-safe pattern)', async () => {
    // This mirrors how Quick Claude sends text to interactive CLI tools:
    // first the text, then a separate Enter keystroke
    await session.writeTextThenEnter('echo INK_SAFE_TEST', 200);
    await session.waitForText('INK_SAFE_TEST', { timeoutMs: 10_000 });

    const result = await session.searchBuffer('INK_SAFE_TEST');
    expect(result.found).toBe(true);
  }, 15_000);

  it('should search with strip_ansi', async () => {
    // Run a command that produces output (cmd.exe itself doesn't add ANSI,
    // but strip_ansi should still work correctly on plain text)
    await session.writeCommand('echo STRIP_TEST_456');
    await session.waitForText('STRIP_TEST_456', { timeoutMs: 10_000 });

    // Search with strip_ansi: true (default)
    const withStrip = await session.searchBuffer('STRIP_TEST_456', true);
    expect(withStrip.found).toBe(true);

    // Search with strip_ansi: false
    const withoutStrip = await session.searchBuffer('STRIP_TEST_456', false);
    expect(withoutStrip.found).toBe(true);
  }, 15_000);
});

describe('quick-claude: grid snapshot', () => {
  let fixture: DaemonFixture;
  let client: DaemonClient;
  let session: SessionHandle;

  beforeAll(async () => {
    fixture = new DaemonFixture({ name: 'qc-grid' });
    await fixture.spawn();
    client = await fixture.connect();
    session = await SessionHandle.create(client, {
      id: 'qc-grid',
      shellType: 'cmd',
      rows: 24,
      cols: 80,
    });
    await session.waitForIdle(500, { timeoutMs: 10_000 });
  }, 20_000);

  afterAll(async () => {
    try { await session.close(); } catch { /* */ }
    client.disconnect();
    await fixture.teardown();
  }, 10_000);

  it('should read grid after command and verify text + cursor', async () => {
    await session.writeCommand('echo GRID_SNAP');
    await session.waitForText('GRID_SNAP', { timeoutMs: 10_000 });
    // Wait a bit for the prompt to reappear after echo
    await session.waitForIdle(300, { timeoutMs: 5_000 });

    const grid = await session.readGrid();

    expect(grid.cols).toBe(80);
    expect(grid.num_rows).toBe(24);
    expect(grid.cursor_row).toBeGreaterThanOrEqual(0);
    expect(grid.cursor_col).toBeGreaterThanOrEqual(0);

    // GRID_SNAP should be visible in the grid rows
    const gridText = grid.rows.join('\n');
    expect(gridText).toContain('GRID_SNAP');
  }, 15_000);

  it('should reflect resize in grid dimensions', async () => {
    await session.resize(30, 120);
    // Give the terminal a moment to process the resize
    await session.waitForIdle(300, { timeoutMs: 5_000 });

    const grid = await session.readGrid();
    expect(grid.cols).toBe(120);
    expect(grid.num_rows).toBe(30);
  }, 10_000);
});
