// lib/discovery.mjs
// Find the MCP HTTP server port from the discovery file.

import { readFileSync } from 'fs';
import { join } from 'path';

const DEFAULT_PORT = 45557;

/**
 * Read the MCP HTTP discovery file to find the server port.
 * Falls back to DEFAULT_PORT if the file doesn't exist or can't be parsed.
 *
 * Discovery file location: %APPDATA%/com.godly.terminal/mcp-http.json
 * Format: { "port": number, "pid": number, "url": string }
 */
export function discoverMcpPort() {
  const appdata = process.env.APPDATA;
  if (!appdata) {
    return { port: DEFAULT_PORT, source: 'default' };
  }

  const discoveryPath = join(appdata, 'com.godly.terminal', 'mcp-http.json');

  try {
    const raw = readFileSync(discoveryPath, 'utf-8');
    const data = JSON.parse(raw);
    if (data.port && typeof data.port === 'number') {
      return { port: data.port, pid: data.pid, url: data.url, source: 'discovery' };
    }
  } catch {
    // File doesn't exist or is invalid
  }

  return { port: DEFAULT_PORT, source: 'default' };
}

/**
 * Check if the MCP server is reachable via GET /health.
 */
export async function checkHealth(port) {
  try {
    const resp = await fetch(`http://127.0.0.1:${port}/health`, {
      signal: AbortSignal.timeout(3000),
    });
    if (resp.ok) {
      return await resp.json();
    }
  } catch {
    // Not reachable
  }
  return null;
}
