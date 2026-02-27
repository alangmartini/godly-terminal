/**
 * DaemonFixture — spawns an isolated daemon per test suite.
 *
 * Mirrors the Rust DaemonFixture pattern from daemon/tests/handler_starvation.rs:
 *   - Unique pipe name per fixture (no production daemon collision)
 *   - GODLY_INSTANCE isolates shim metadata directory
 *   - GODLY_NO_DETACH=1 keeps daemon as child process for cleanup
 *   - Kill by child.kill(), never by process name
 */

import { spawn, type ChildProcess } from 'node:child_process';
import path from 'node:path';
import fs from 'node:fs';
import { DaemonClient } from './daemon-client.js';

const DAEMON_BINARY = path.resolve('src-tauri/target/debug/godly-daemon.exe');

export interface DaemonFixtureOptions {
  /** Human-readable name for the fixture (used in pipe name). */
  name: string;
  /** Extra environment variables for the daemon process. */
  env?: Record<string, string>;
}

export class DaemonFixture {
  readonly pipeName: string;
  readonly instanceName: string;
  private child: ChildProcess | null = null;
  private _started = false;

  constructor(options: DaemonFixtureOptions) {
    const suffix = `${options.name}-${process.pid}-${Date.now()}`;
    this.instanceName = `test-${suffix}`;
    this.pipeName = `\\\\.\\pipe\\godly-test-${suffix}`;
  }

  /**
   * Spawn the daemon and wait until it's ready (responds to Ping).
   */
  async spawn(): Promise<void> {
    if (!fs.existsSync(DAEMON_BINARY)) {
      throw new Error(
        `Daemon binary not found at ${DAEMON_BINARY}. Run 'npm run build:daemon' first.`,
      );
    }

    this.child = spawn(DAEMON_BINARY, [], {
      env: {
        ...process.env,
        GODLY_PIPE_NAME: this.pipeName,
        GODLY_INSTANCE: this.instanceName,
        GODLY_NO_DETACH: '1',
        RUST_LOG: 'warn',
      },
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    // Capture stderr for debugging test failures
    let stderr = '';
    this.child.stderr?.on('data', (chunk: Buffer) => {
      stderr += chunk.toString();
    });

    this.child.on('exit', (code) => {
      if (this._started && code !== null && code !== 0) {
        console.error(`[DaemonFixture] daemon exited with code ${code}`);
        if (stderr) console.error(`[DaemonFixture] stderr: ${stderr.slice(-500)}`);
      }
    });

    // Wait for daemon to be ready by pinging it
    const client = new DaemonClient({ timeout: 5000 });
    try {
      await client.connect(this.pipeName, 50, 100); // 50 retries × 100ms = 5s
      const resp = await client.sendRequest({ type: 'Ping' });
      if (resp.type !== 'Pong') {
        throw new Error(`Expected Pong, got ${resp.type}`);
      }
      this._started = true;
    } finally {
      client.disconnect();
    }
  }

  /**
   * Create a new DaemonClient connected to this fixture's pipe.
   */
  async connect(): Promise<DaemonClient> {
    const client = new DaemonClient();
    await client.connect(this.pipeName);
    return client;
  }

  /**
   * Kill the daemon process and clean up.
   */
  async teardown(): Promise<void> {
    if (this.child) {
      this.child.kill();
      await new Promise<void>((resolve) => {
        const timer = setTimeout(() => {
          // Force kill if graceful didn't work
          try {
            this.child?.kill('SIGKILL');
          } catch {
            // Already dead
          }
          resolve();
        }, 3000);

        this.child!.on('exit', () => {
          clearTimeout(timer);
          resolve();
        });
      });
      this.child = null;
    }
    this._started = false;
  }
}
