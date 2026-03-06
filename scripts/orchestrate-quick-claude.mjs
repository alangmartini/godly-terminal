import { McpClient } from './mcp-client.mjs';

function parseToolResult(result) {
  const txt = result?.content?.[0]?.text;
  if (!txt) return result;
  try {
    return JSON.parse(txt);
  } catch {
    return { raw: txt };
  }
}

const WORKSPACE_PATH = 'C:\\Users\\alanm\\Documents\\dev\\godly-claude\\godly-terminal';

const agents = [
  {
    name: 'lane-a-contract',
    branch: 'native-lane-a-contract',
    prompt: `TASK: Implement Lane A (contract freeze) initial deliverable for native migration.\nSCOPE: Extract and document frontend-backend contract from existing TS service usage.\nACCEPTANCE CRITERIA:\n- Create migration/frontend_contract_v1.md with sections: terminal lifecycle, terminal I/O, workspace ops, wait/polling, split/layout, errors/retries.\n- Include concrete request/response examples for create_terminal, execute_command, read_grid, wait_for_idle, list_workspaces.\n- Add a short invariants section (idempotency, newline conversion, timeout semantics).\nCONSTRAINTS:\n- Work only in this repo/worktree.\n- Do not modify unrelated files.\n- Keep docs concise and implementation-focused.\nDO NOT: start native UI scaffolding in this lane.`
  },
  {
    name: 'lane-b-shell',
    branch: 'native-lane-b-shell',
    prompt: `TASK: Implement Lane B initial scaffold for Iced native shell.\nSCOPE: Add non-invasive src-native scaffold and minimal cargo wiring, without changing current web runtime.\nACCEPTANCE CRITERIA:\n- Add src-native/README.md describing crate layout and responsibilities.\n- Create empty crate scaffolds: src-native/iced-shell, src-native/app-adapter, src-native/terminal-surface with minimal Cargo.toml + src/lib.rs.\n- Ensure no existing npm/tauri dev flow is broken.\n- Add brief TODO markers for next implementation steps in each crate.\nCONSTRAINTS:\n- Do not remove or alter current TS frontend code paths.\n- Keep compile impact isolated.\nDO NOT: implement full rendering; only safe scaffolding.`
  },
  {
    name: 'lane-h-parity-ci',
    branch: 'native-lane-h-parity-ci',
    prompt: `TASK: Implement Lane H kickoff artifacts for parity/perf validation.\nSCOPE: Add CI/docs skeleton for native migration quality gates.\nACCEPTANCE CRITERIA:\n- Add migration/native-release-gates.md with measurable gate definitions (stability, perf, parity, persistence, rollback).\n- Add .github/workflows/native-shadow.yml skeleton job(s) that currently run no-op checks with TODO comments and clear names for future native build/parity tests.\n- Keep workflow disabled from blocking current pipeline (non-required, lightweight).\nCONSTRAINTS:\n- No heavy CI steps yet.\n- Keep changes additive and safe.\nDO NOT: alter existing production workflow behavior.`
  }
];

const client = new McpClient(8089);

(async () => {
  await client.connect();

  const wsRes = parseToolResult(await client.callTool('list_workspaces', {}));
  const workspaces = wsRes.workspaces || [];
  const targetWs = workspaces.find(w => w.folder_path?.toLowerCase() === WORKSPACE_PATH.toLowerCase())
    || workspaces.find(w => (w.name || '').toLowerCase() === 'godly-terminal')
    || workspaces[0];

  if (!targetWs?.id) {
    throw new Error('Could not resolve target workspace_id');
  }

  const out = [];

  for (const a of agents) {
    const res = parseToolResult(await client.callTool('quick_claude', {
      workspace_id: targetWs.id,
      prompt: a.prompt,
      branch_name: a.branch,
      skip_fetch: true
    }));

    const terminalId = res.terminal_id || res.id || null;
    out.push({
      agent: a.name,
      workspace_id: targetWs.id,
      workspace_name: targetWs.name,
      terminal_id: terminalId,
      worktree_branch: res.worktree_branch || a.branch,
      raw: res
    });
  }

  // Optional: quick snapshot of terminals for confirmation
  const terms = parseToolResult(await client.callTool('list_terminals', {}));

  console.log(JSON.stringify({
    workspace: targetWs,
    launched: out,
    terminals_count: (terms.terminals || []).length
  }, null, 2));

  await client.close();
})().catch(async (err) => {
  console.error(err?.stack || err?.message || String(err));
  try { await client.close(); } catch {}
  process.exit(1);
});
