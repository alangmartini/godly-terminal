import { EventSource } from 'eventsource';

class McpClientLong {
  constructor(port = 8089, callTimeoutMs = 180000) {
    this.baseUrl = `http://127.0.0.1:${port}`;
    this.callTimeoutMs = callTimeoutMs;
    this.endpointPath = null;
    this.eventSource = null;
    this.pending = new Map();
    this.nextId = 1;
  }
  async connect() {
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => reject(new Error('connect timeout')), 15000);
      this.eventSource = new EventSource(`${this.baseUrl}/sse`);
      this.eventSource.addEventListener('endpoint', (e) => { this.endpointPath = e.data; clearTimeout(t); resolve(); });
      this.eventSource.addEventListener('message', (e) => {
        try {
          const msg = JSON.parse(e.data);
          const p = this.pending.get(msg.id);
          if (!p) return;
          this.pending.delete(msg.id);
          if (msg.error) p.reject(new Error(msg.error.message || 'error')); else p.resolve(msg.result);
        } catch {}
      });
    });
  }
  async callTool(name, args = {}) {
    const id = this.nextId++;
    const body = JSON.stringify({ jsonrpc: '2.0', id, method: 'tools/call', params: { name, arguments: args } });
    const url = `${this.baseUrl}${this.endpointPath}`;
    return new Promise((resolve, reject) => {
      const t = setTimeout(() => { this.pending.delete(id); reject(new Error(`timeout ${name}`)); }, this.callTimeoutMs);
      this.pending.set(id, {
        resolve: (r) => { clearTimeout(t); resolve(r); },
        reject: (e) => { clearTimeout(t); reject(e); },
      });
      fetch(url, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body }).catch((e) => {
        clearTimeout(t); this.pending.delete(id); reject(e);
      });
    });
  }
  async close() { if (this.eventSource) this.eventSource.close(); }
}

const parse = (r) => { const t = r?.content?.[0]?.text; try { return JSON.parse(t); } catch { return { raw: t }; } };

const client = new McpClientLong(8089, 180000);
(async () => {
  await client.connect();
  const ws = parse(await client.callTool('list_workspaces', {}));
  const target = (ws.workspaces || []).find(w => w.name === 'godly-terminal') || ws.workspaces?.[0];
  const command = "function dcodex { param([Parameter(ValueFromRemainingArguments=$true)][string[]]$a) codex --dangerously-bypass-approvals-and-sandbox @a }; dcodex exec 'TASK: Lane A dcodex pass. Review migration/frontend_contract_v1.md and refine missing sections if needed. Keep scope to this file only.'";
  const created = parse(await client.callTool('create_terminal', {
    workspace_id: target.id,
    worktree_name: 'native-lane-a-contract-dcodex',
    command,
  }));
  await new Promise(r => setTimeout(r, 3000));
  const out = parse(await client.callTool('read_terminal', { terminal_id: created.id, mode: 'tail', lines: 120, strip_ansi: true }));
  const txt = out.content || '';
  console.log(JSON.stringify({
    workspace: target,
    terminal_id: created.id,
    worktree_branch: created.worktree_branch,
    worktree_path: created.worktree_path,
    started_hint: txt.includes('codex') || txt.includes('Codex') || txt.includes('TASK:'),
    tail_excerpt: txt.slice(-500)
  }, null, 2));
  await client.close();
})().catch(async (e) => { console.error(e?.stack || e?.message || String(e)); try { await client.close(); } catch {} process.exit(1); });
