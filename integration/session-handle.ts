/**
 * SessionHandle — high-level API wrapping DaemonClient for a single session.
 *
 * Provides ergonomic methods for writing, waiting, and reading terminal state.
 */

import type { DaemonClient } from './daemon-client.js';
import type { ShellType, SessionInfo, GridData, Response } from './protocol.js';

export interface CreateSessionOptions {
  id?: string;
  shellType?: ShellType;
  rows?: number;
  cols?: number;
  cwd?: string;
  env?: Record<string, string>;
}

export interface WaitOptions {
  /** Timeout in ms. Default: 30000 */
  timeoutMs?: number;
  /** Poll interval in ms. Default: 200 */
  pollMs?: number;
}

export class SessionHandle {
  readonly sessionId: string;
  readonly client: DaemonClient;

  private constructor(client: DaemonClient, sessionId: string) {
    this.client = client;
    this.sessionId = sessionId;
  }

  /**
   * Create a new session and attach to it.
   */
  static async create(client: DaemonClient, options?: CreateSessionOptions): Promise<SessionHandle> {
    const id = options?.id ?? `test-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    const shellType = options?.shellType ?? 'cmd';
    const rows = options?.rows ?? 24;
    const cols = options?.cols ?? 80;

    const resp = await client.sendRequest({
      type: 'CreateSession',
      id,
      shell_type: shellType,
      rows,
      cols,
      cwd: options?.cwd,
      env: options?.env,
    });

    if (resp.type === 'Error') {
      throw new Error(`CreateSession failed: ${resp.message}`);
    }
    if (resp.type !== 'SessionCreated') {
      throw new Error(`Unexpected response: ${resp.type}`);
    }

    // Attach to receive events
    const attachResp = await client.sendRequest({ type: 'Attach', session_id: id });
    // Attach returns Ok or Buffer (initial replay)
    if (attachResp.type === 'Error') {
      throw new Error(`Attach failed: ${attachResp.message}`);
    }

    return new SessionHandle(client, id);
  }

  /**
   * Write raw text to the terminal (no newline appended).
   */
  async write(text: string): Promise<void> {
    const data = Array.from(Buffer.from(text, 'utf-8'));
    const resp = await this.client.sendRequest({
      type: 'Write',
      session_id: this.sessionId,
      data,
    });
    assertOk(resp);
  }

  /**
   * Write a command (text + carriage return).
   */
  async writeCommand(cmd: string): Promise<void> {
    await this.write(cmd + '\r');
  }

  /**
   * Write text, wait briefly, then send Enter separately.
   * This is the ink-safe pattern for interactive CLI tools
   * that process input character-by-character.
   */
  async writeTextThenEnter(text: string, delayMs = 200): Promise<void> {
    await this.write(text);
    await sleep(delayMs);
    await this.write('\r');
  }

  /**
   * Wait until the session has been idle (no output) for the given duration.
   */
  async waitForIdle(idleMs: number, options?: WaitOptions): Promise<void> {
    const timeout = options?.timeoutMs ?? 30_000;
    const poll = options?.pollMs ?? 200;
    const deadline = Date.now() + timeout;

    while (Date.now() < deadline) {
      const resp = await this.client.sendRequest({
        type: 'GetLastOutputTime',
        session_id: this.sessionId,
      });

      if (resp.type === 'LastOutputTime') {
        const elapsed = Date.now() - resp.epoch_ms;
        if (elapsed >= idleMs) return;
      }

      await sleep(poll);
    }

    throw new Error(`waitForIdle(${idleMs}ms) timed out after ${timeout}ms`);
  }

  /**
   * Wait until the given text appears in the session's buffer.
   */
  async waitForText(needle: string, options?: WaitOptions & { stripAnsi?: boolean }): Promise<void> {
    const timeout = options?.timeoutMs ?? 30_000;
    const poll = options?.pollMs ?? 200;
    const stripAnsi = options?.stripAnsi ?? true;
    const deadline = Date.now() + timeout;

    while (Date.now() < deadline) {
      const resp = await this.client.sendRequest({
        type: 'SearchBuffer',
        session_id: this.sessionId,
        text: needle,
        strip_ansi: stripAnsi,
      });

      if (resp.type === 'SearchResult' && resp.found) return;

      await sleep(poll);
    }

    throw new Error(`waitForText("${needle}") timed out after ${timeout}ms`);
  }

  /**
   * Search the session buffer for text. Returns immediately.
   */
  async searchBuffer(text: string, stripAnsi = true): Promise<{ found: boolean; running: boolean }> {
    const resp = await this.client.sendRequest({
      type: 'SearchBuffer',
      session_id: this.sessionId,
      text,
      strip_ansi: stripAnsi,
    });

    if (resp.type !== 'SearchResult') {
      throw new Error(`Unexpected response: ${resp.type}`);
    }

    return { found: resp.found, running: resp.running };
  }

  /**
   * Read the plain-text grid snapshot.
   */
  async readGrid(): Promise<GridData> {
    const resp = await this.client.sendRequest({
      type: 'ReadGrid',
      session_id: this.sessionId,
    });

    if (resp.type === 'Error') {
      throw new Error(`ReadGrid failed: ${resp.message}`);
    }
    if (resp.type !== 'Grid') {
      throw new Error(`Unexpected response: ${resp.type}`);
    }

    return resp.grid;
  }

  /**
   * Resize the terminal.
   */
  async resize(rows: number, cols: number): Promise<void> {
    const resp = await this.client.sendRequest({
      type: 'Resize',
      session_id: this.sessionId,
      rows,
      cols,
    });
    assertOk(resp);
  }

  /**
   * Detach from the session (session keeps running).
   */
  async detach(): Promise<void> {
    const resp = await this.client.sendRequest({
      type: 'Detach',
      session_id: this.sessionId,
    });
    assertOk(resp);
  }

  /**
   * Close the session (kills the shell process).
   */
  async close(): Promise<void> {
    const resp = await this.client.sendRequest({
      type: 'CloseSession',
      session_id: this.sessionId,
    });
    assertOk(resp);
  }
}

function assertOk(resp: Response): void {
  if (resp.type === 'Error') {
    throw new Error(`Daemon error: ${resp.message}`);
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
