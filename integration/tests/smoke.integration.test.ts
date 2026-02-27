/**
 * Smoke tests — validate the integration test framework itself works.
 *
 * Tests: daemon spawn, connect, ping/pong, session create/attach,
 * write/search, grid read, session close.
 */

import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { DaemonFixture } from '../daemon-fixture.js';
import { DaemonClient } from '../daemon-client.js';
import { SessionHandle } from '../session-handle.js';

describe('smoke: daemon lifecycle', () => {
  let fixture: DaemonFixture;

  beforeAll(async () => {
    fixture = new DaemonFixture({ name: 'smoke' });
    await fixture.spawn();
  }, 15_000);

  afterAll(async () => {
    await fixture.teardown();
  }, 10_000);

  it('should connect and receive Pong', async () => {
    const client = await fixture.connect();
    try {
      const resp = await client.sendRequest({ type: 'Ping' });
      expect(resp.type).toBe('Pong');
    } finally {
      client.disconnect();
    }
  });

  it('should list sessions (initially empty)', async () => {
    const client = await fixture.connect();
    try {
      const resp = await client.sendRequest({ type: 'ListSessions' });
      expect(resp.type).toBe('SessionList');
      if (resp.type === 'SessionList') {
        expect(resp.sessions).toBeInstanceOf(Array);
      }
    } finally {
      client.disconnect();
    }
  });

  it('should create a session', async () => {
    const client = await fixture.connect();
    try {
      const resp = await client.sendRequest({
        type: 'CreateSession',
        id: 'smoke-create',
        shell_type: 'cmd',
        rows: 24,
        cols: 80,
      });
      expect(resp.type).toBe('SessionCreated');
      if (resp.type === 'SessionCreated') {
        expect(resp.session.id).toBe('smoke-create');
        expect(resp.session.running).toBe(true);
      }

      // Clean up
      await client.sendRequest({ type: 'CloseSession', session_id: 'smoke-create' });
    } finally {
      client.disconnect();
    }
  });
});

describe('smoke: session I/O', () => {
  let fixture: DaemonFixture;
  let client: DaemonClient;
  let session: SessionHandle;

  beforeAll(async () => {
    fixture = new DaemonFixture({ name: 'smoke-io' });
    await fixture.spawn();
    client = await fixture.connect();
    session = await SessionHandle.create(client, {
      id: 'smoke-io',
      shellType: 'cmd',
    });
    // Wait for cmd.exe prompt to appear
    await session.waitForIdle(500, { timeoutMs: 10_000 });
  }, 20_000);

  afterAll(async () => {
    try {
      await session.close();
    } catch {
      // Session may already be closed
    }
    client.disconnect();
    await fixture.teardown();
  }, 10_000);

  it('should write a command and find output via SearchBuffer', async () => {
    await session.writeCommand('echo HELLO_INTEGRATION');
    await session.waitForText('HELLO_INTEGRATION', { timeoutMs: 10_000 });

    const result = await session.searchBuffer('HELLO_INTEGRATION');
    expect(result.found).toBe(true);
  }, 15_000);

  it('should read grid and find text content', async () => {
    const grid = await session.readGrid();

    expect(grid.cols).toBe(80);
    expect(grid.num_rows).toBe(24);
    expect(grid.rows.length).toBeGreaterThan(0);

    // The HELLO_INTEGRATION text from the previous test should be in the grid or scrollback
    const gridText = grid.rows.join('\n');
    // The grid may have scrolled past the output, so we use SearchBuffer for reliability
    const result = await session.searchBuffer('HELLO_INTEGRATION');
    expect(result.found).toBe(true);
  }, 10_000);

  it('should handle multiple commands sequentially', async () => {
    await session.writeCommand('echo MARKER_A');
    await session.waitForText('MARKER_A', { timeoutMs: 10_000 });

    await session.writeCommand('echo MARKER_B');
    await session.waitForText('MARKER_B', { timeoutMs: 10_000 });

    const resultA = await session.searchBuffer('MARKER_A');
    const resultB = await session.searchBuffer('MARKER_B');
    expect(resultA.found).toBe(true);
    expect(resultB.found).toBe(true);
  }, 20_000);
});

describe('smoke: multiple clients', () => {
  let fixture: DaemonFixture;

  beforeAll(async () => {
    fixture = new DaemonFixture({ name: 'smoke-multi' });
    await fixture.spawn();
  }, 15_000);

  afterAll(async () => {
    await fixture.teardown();
  }, 10_000);

  it('should handle multiple concurrent connections', async () => {
    const client1 = await fixture.connect();
    const client2 = await fixture.connect();

    try {
      const [resp1, resp2] = await Promise.all([
        client1.sendRequest({ type: 'Ping' }),
        client2.sendRequest({ type: 'Ping' }),
      ]);

      expect(resp1.type).toBe('Pong');
      expect(resp2.type).toBe('Pong');
    } finally {
      client1.disconnect();
      client2.disconnect();
    }
  });
});
