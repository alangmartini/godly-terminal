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

const jobs = [
  {
    id: '8c5604bd-f8e3-41d1-b1bc-9ef46dcdce73',
    msg: 'TASK: Implement Lane A contract doc kickoff. Create migration/frontend_contract_v1.md with concrete MCP/IPC contracts and invariants. Keep scope minimal and focused.\n'
  },
  {
    id: 'a9e14e28-547e-4c88-9e4b-092dc34a4ea4',
    msg: 'TASK: Implement Lane B scaffold kickoff. Create src-native crate scaffolds (iced-shell, app-adapter, terminal-surface) and src-native/README.md. Keep current web path untouched.\n'
  },
  {
    id: '4ece888c-aefe-41b1-a41e-709b81730dc7',
    msg: 'TASK: Implement Lane H kickoff. Add migration/native-release-gates.md and .github/workflows/native-shadow.yml skeleton (non-blocking).\n'
  }
];

const client = new McpClient(8089);

(async () => {
  await client.connect();
  for (const j of jobs) {
    const res = parseToolResult(await client.callTool('write_to_terminal', {
      terminal_id: j.id,
      data: j.msg,
    }));
    console.log(JSON.stringify({ terminal_id: j.id, result: res }));
  }
  await client.close();
})().catch(async (e) => {
  console.error(e?.stack || e?.message || String(e));
  try { await client.close(); } catch {}
  process.exit(1);
});
