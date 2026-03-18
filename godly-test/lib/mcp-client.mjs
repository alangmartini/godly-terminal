// lib/mcp-client.mjs
// HTTP Streamable MCP client for godly-terminal.
//
// Protocol:
//   1. POST /mcp with method "initialize" → get mcp-session-id header
//   2. POST /mcp with mcp-session-id header for all subsequent calls
//   3. JSON-RPC 2.0 request/response

export class McpClient {
  constructor(port) {
    this.baseUrl = `http://127.0.0.1:${port}`;
    this.sessionId = null;
    this._nextId = 1;
  }

  async connect() {
    const id = this._nextId++;
    const resp = await fetch(`${this.baseUrl}/mcp`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        jsonrpc: '2.0',
        id,
        method: 'initialize',
        params: {
          protocolVersion: '2024-11-05',
          capabilities: {},
          clientInfo: { name: 'godly-test', version: '0.1.0' },
        },
      }),
    });

    if (!resp.ok) {
      throw new Error(`MCP initialize failed: HTTP ${resp.status}`);
    }

    this.sessionId = resp.headers.get('mcp-session-id');
    if (!this.sessionId) {
      throw new Error('MCP server did not return mcp-session-id header');
    }

    const result = await resp.json();
    if (result.error) {
      throw new Error(`MCP initialize error: ${result.error.message}`);
    }

    // Send initialized notification (no id = notification, server returns 202)
    await fetch(`${this.baseUrl}/mcp`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'mcp-session-id': this.sessionId,
      },
      body: JSON.stringify({
        jsonrpc: '2.0',
        method: 'notifications/initialized',
      }),
    });

    return result.result;
  }

  async callTool(name, args = {}, { timeout = 60000 } = {}) {
    if (!this.sessionId) {
      throw new Error('Not connected — call connect() first');
    }

    const id = this._nextId++;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeout);

    try {
      const resp = await fetch(`${this.baseUrl}/mcp`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          'mcp-session-id': this.sessionId,
        },
        body: JSON.stringify({
          jsonrpc: '2.0',
          id,
          method: 'tools/call',
          params: { name, arguments: args },
        }),
        signal: controller.signal,
      });

      if (!resp.ok) {
        const text = await resp.text().catch(() => '');
        throw new Error(`MCP tool '${name}' HTTP ${resp.status}: ${text}`);
      }

      const result = await resp.json();
      if (result.error) {
        throw new Error(`MCP tool '${name}' error: ${result.error.message}`);
      }

      return result.result;
    } finally {
      clearTimeout(timer);
    }
  }

  async close() {
    if (!this.sessionId) return;

    try {
      await fetch(`${this.baseUrl}/mcp`, {
        method: 'DELETE',
        headers: { 'mcp-session-id': this.sessionId },
        signal: AbortSignal.timeout(5000),
      });
    } catch {
      // Best-effort cleanup
    }

    this.sessionId = null;
  }
}

/** Parse MCP tool result content — extract text or JSON from content array */
export function parseToolResult(result) {
  if (!result) return {};
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
