// scripts/mcp-client.mjs
// Lightweight MCP SSE client for programmatic access to godly-mcp tools.
//
// Protocol:
//   1. GET /sse → SSE stream, first event = "endpoint" with POST URL
//   2. POST /messages?sessionId=X with JSON-RPC { method: "tools/call", params: { name, arguments } }
//   3. Response arrives via SSE "message" event

import { EventSource } from 'eventsource';

export class McpClient {
  constructor(port = 8089) {
    this.baseUrl = `http://127.0.0.1:${port}`;
    this.sessionId = null;
    this.endpointPath = null;
    this.eventSource = null;
    this._pendingRequests = new Map(); // id → { resolve, reject }
    this._nextId = 1;
  }

  async connect() {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        reject(new Error('MCP SSE connection timed out after 10s'));
      }, 10000);

      this.eventSource = new EventSource(`${this.baseUrl}/sse`);

      this.eventSource.addEventListener('endpoint', (event) => {
        this.endpointPath = event.data;
        // Extract sessionId from endpoint URL
        const match = this.endpointPath.match(/sessionId=([^&]+)/);
        if (match) {
          this.sessionId = match[1];
        }
        clearTimeout(timeout);
        resolve();
      });

      this.eventSource.addEventListener('message', (event) => {
        try {
          const response = JSON.parse(event.data);
          const pending = this._pendingRequests.get(response.id);
          if (pending) {
            this._pendingRequests.delete(response.id);
            if (response.error) {
              pending.reject(new Error(`MCP error ${response.error.code}: ${response.error.message}`));
            } else {
              pending.resolve(response.result);
            }
          }
        } catch (e) {
          // Ignore malformed SSE messages
        }
      });

      this.eventSource.onerror = (err) => {
        clearTimeout(timeout);
        if (!this.sessionId) {
          reject(new Error(`MCP SSE connection failed: ${err.message || 'unknown error'}`));
        }
      };
    });
  }

  async callTool(name, args = {}) {
    if (!this.endpointPath) {
      throw new Error('Not connected — call connect() first');
    }

    const id = this._nextId++;
    const body = JSON.stringify({
      jsonrpc: '2.0',
      id,
      method: 'tools/call',
      params: { name, arguments: args },
    });

    const postUrl = `${this.baseUrl}${this.endpointPath}`;

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this._pendingRequests.delete(id);
        reject(new Error(`MCP tool call '${name}' timed out after 30s`));
      }, 30000);

      this._pendingRequests.set(id, {
        resolve: (result) => {
          clearTimeout(timeout);
          resolve(result);
        },
        reject: (err) => {
          clearTimeout(timeout);
          reject(err);
        },
      });

      fetch(postUrl, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      }).catch((err) => {
        clearTimeout(timeout);
        this._pendingRequests.delete(id);
        reject(new Error(`MCP POST failed: ${err.message}`));
      });
    });
  }

  async close() {
    if (this.eventSource) {
      this.eventSource.close();
      this.eventSource = null;
    }
    // Reject any pending requests
    for (const [id, pending] of this._pendingRequests) {
      pending.reject(new Error('Client closed'));
    }
    this._pendingRequests.clear();
  }
}
