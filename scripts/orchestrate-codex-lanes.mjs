import { EventSource } from 'eventsource';

class McpClientLong {
  constructor(port = 8089, callTimeoutMs = 180000) {
    this.baseUrl = `http://127.0.0.1:${port}`;
    this.callTimeoutMs = callTimeoutMs;
    this.sessionId = null;
    this.endpointPath = null;
    this.eventSource = null;
    this._pending = new Map();
    this._id = 1;
  }

  async connect() {
    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('SSE connect timeout')), 15000);
      this.eventSource = new EventSource(`${this.baseUrl}/sse`);
      this.eventSource.addEventListener('endpoint', (event) => {
        this.endpointPath = event.data;
        const m = this.endpointPath.match(/sessionId=([^&]+)/);
        if (m) this.sessionId = m[1];
        clearTimeout(timeout);
        resolve();
      });
      this.eventSource.addEventListener('message', (event) => {
        try {
          const msg = JSON.parse(event.data);
          const p = this._pending.get(msg.id);
          if (!p) return;
          this._pending.delete(msg.id);
          if (msg.error) p.reject(new Error(msg.error.message || 'MCP error'));
          else p.resolve(msg.result);
        } catch {}
      });
      this.eventSource.onerror = () => {
        if (!this.sessionId) {
          clearTimeout(timeout);
          reject(new Error('SSE error before session established'));
        }
      };
    });
  }

  async callTool(name, args = {}) {
    if (!this.endpointPath) throw new Error('Not connected');
    const id = this._id++;
    const url = `${this.baseUrl}${this.endpointPath}`;
    const body = JSON.stringify({
      jsonrpc: '2.0',
      id,
      method: 'tools/call',
      params: { name, arguments: args },
    });

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        this._pending.delete(id);
        reject(new Error(`MCP call timeout for ${name} after ${this.callTimeoutMs}ms`));
      }, this.callTimeoutMs);

      this._pending.set(id, {
        resolve: (r) => { clearTimeout(timeout); resolve(r); },
        reject: (e) => { clearTimeout(timeout); reject(e); },
      });

      fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body,
      }).catch((err) => {
        clearTimeout(timeout);
        this._pending.delete(id);
        reject(err);
      });
    });
  }

  async close() {
    if (this.eventSource) this.eventSource.close();
    for (const [, p] of this._pending) p.reject(new Error('client closed'));
    this._pending.clear();
  }
}

function parseToolResult(result) {
  const txt = result?.content?.[0]?.text;
  if (!txt) return result;
  try { return JSON.parse(txt); } catch { return { raw: txt }; }
}

const WORKSPACE_PATH = 'C:\\Users\\alanm\\Documents\\dev\\godly-claude\\godly-terminal';

const client = new McpClientLong(8089, 180000);

(async () => {
  await client.connect();
  const wsRes = parseToolResult(await client.callTool('list_workspaces', {}));
  const workspaces = wsRes.workspaces || [];
  const ws = workspaces.find(w => (w.folder_path || '').toLowerCase() === WORKSPACE_PATH.toLowerCase())
    || workspaces.find(w => (w.name || '').toLowerCase() === 'godly-terminal')
    || workspaces[0];

  if (!ws?.id) throw new Error('Workspace not found');

  const lanes = [
    {
      agent: 'lane-b-codex',
      worktree: 'native-lane-b-shell-codex',
      command: "codex exec --dangerously-bypass-approvals-and-sandbox 'TASK: Lane B kickoff. Create src-native crate scaffolds (iced-shell, app-adapter, terminal-surface) and src-native/README.md. Keep existing web runtime untouched.'"
    },
    {
      agent: 'lane-h-codex',
      worktree: 'native-lane-h-parity-codex',
      command: "codex exec --dangerously-bypass-approvals-and-sandbox 'TASK: Lane H kickoff. Add migration/native-release-gates.md and .github/workflows/native-shadow.yml skeleton with non-blocking placeholder jobs.'"
    }
  ];

  const launched = [];
  for (const lane of lanes) {
    const created = parseToolResult(await client.callTool('create_terminal', {
      workspace_id: ws.id,
      worktree_name: lane.worktree,
      command: lane.command,
    }));

    const terminalId = created.id;

    await new Promise(r => setTimeout(r, 2500));

    const tail = parseToolResult(await client.callTool('read_terminal', {
      terminal_id: terminalId,
      mode: 'tail',
      lines: 120,
      strip_ansi: true,
    }));

    const content = tail.content || '';
    launched.push({
      agent: lane.agent,
      terminal_id: terminalId,
      worktree_branch: created.worktree_branch || null,
      worktree_path: created.worktree_path || null,
      command_started_hint: content.includes('codex exec') || content.includes('Codex CLI') || content.includes('TASK:'),
      tail_excerpt: content.slice(-500),
    });
  }

  console.log(JSON.stringify({ workspace: ws, launched }, null, 2));
  await client.close();
})().catch(async (err) => {
  console.error(err?.stack || err?.message || String(err));
  try { await client.close(); } catch {}
  process.exit(1);
});
