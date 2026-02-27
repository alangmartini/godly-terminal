/**
 * Bug #419: MCP execute_command cannot detect interactive prompts.
 *
 * When a command produces an interactive prompt (yes/no, selection menu,
 * text input), execute_command's idle detection treats the prompt as
 * "command completed" because the terminal goes silent while waiting for
 * user input.
 *
 * Fix: The daemon now computes an `input_expected` heuristic based on
 * cursor position and VT grid state, and returns it in LastOutputTime.
 * execute_command uses this to return completed=false when the terminal
 * is idle but waiting for input.
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { DaemonFixture } from '../daemon-fixture.js';
import { DaemonClient } from '../daemon-client.js';
import { SessionHandle } from '../session-handle.js';

describe('Bug #419: execute_command and interactive prompts', () => {
  let fixture: DaemonFixture;
  let client: DaemonClient;
  let session: SessionHandle;

  beforeAll(async () => {
    fixture = new DaemonFixture({ name: 'interactive-prompt' });
    await fixture.spawn();
    client = await fixture.connect();
    session = await SessionHandle.create(client, {
      id: 'prompt-test',
      shellType: 'cmd',
    });
    // Wait for cmd.exe prompt to be ready
    await session.waitForIdle(1000, { timeoutMs: 10_000 });
  }, 20_000);

  afterAll(async () => {
    try {
      // Send Ctrl+C then exit to clean up any dangling prompts
      await session.write('\x03\r');
      await session.writeCommand('exit');
    } catch {
      // Best effort
    }
    try {
      await session.close();
    } catch {
      // Session may already be closed
    }
    client.disconnect();
    await fixture.teardown();
  }, 10_000);

  /**
   * Core fix: GetLastOutputTime now includes input_expected=true when
   * the terminal cursor is at a non-prompt position (waiting for input).
   */
  it('should report input_expected=true when command prompts for input', async () => {
    // Send a command that displays a prompt and waits for input
    // cmd.exe: `set /p VAR=prompt_text` prints prompt_text then blocks on stdin
    await session.writeCommand('set /p ANSWER=BUG419_PROMPT: ');

    // Wait for the prompt text to appear on screen
    await session.waitForText('BUG419_PROMPT', { timeoutMs: 10_000 });

    // Wait for idle — same as execute_command's idle detection (2s default)
    await session.waitForIdle(2000, { timeoutMs: 10_000 });

    // Query daemon state — this is what execute_command uses to decide "completed"
    const resp = await client.sendRequest({
      type: 'GetLastOutputTime',
      session_id: session.sessionId,
    });

    expect(resp.type).toBe('LastOutputTime');
    if (resp.type !== 'LastOutputTime') return;

    // The process IS still running — it's waiting for input, not finished
    expect(resp.running).toBe(true);

    // The terminal has been idle for >= 2s (prompt displayed, waiting for keystroke)
    const elapsed = Date.now() - resp.epoch_ms;
    expect(elapsed).toBeGreaterThanOrEqual(2000);

    // FIX: The daemon now reports input_expected=true
    expect(resp.input_expected).toBe(true);

    // Verify the grid shows the prompt — the user sees an input prompt
    const grid = await session.readGrid();
    const gridText = grid.rows.join('\n');
    expect(gridText).toContain('BUG419_PROMPT');

    // Clean up: answer the prompt to unblock it
    await session.write('y\r');
    await session.waitForIdle(1000, { timeoutMs: 5_000 });
  }, 30_000);

  /**
   * End-to-end fix: when execute_command detects input_expected, it can
   * return completed=false, allowing Claude to detect the prompt, answer it,
   * and then send the next command to the shell.
   */
  it('should allow detecting prompt and answering before next command', async () => {
    // Send an interactive command
    await session.writeCommand('set /p X=DANGLING_PROMPT: ');
    await session.waitForText('DANGLING_PROMPT', { timeoutMs: 10_000 });

    // execute_command waits for idle (2s)
    await session.waitForIdle(2000, { timeoutMs: 10_000 });

    // Check that daemon reports input_expected
    const statusResp = await client.sendRequest({
      type: 'GetLastOutputTime',
      session_id: session.sessionId,
    });
    expect(statusResp.type).toBe('LastOutputTime');
    if (statusResp.type === 'LastOutputTime') {
      expect(statusResp.input_expected).toBe(true);
    }

    // Step 2: Because input_expected=true, Claude knows to answer the prompt first
    await session.write('done\r');
    await session.waitForIdle(1500, { timeoutMs: 5_000 });

    // Step 3: NOW send the next command — it goes to the shell, not the prompt
    await session.writeCommand('echo SHOULD_EXECUTE_419');
    await session.waitForText('SHOULD_EXECUTE_419', { timeoutMs: 10_000 });

    // The echo command actually executed because we answered the prompt first
    // Use searchBuffer to search the full output buffer, not just the visible grid
    const result = await session.searchBuffer('SHOULD_EXECUTE_419');
    expect(result.found).toBe(true);
  }, 30_000);
});
