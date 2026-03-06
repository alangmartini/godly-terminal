#!/usr/bin/env node
// Quick test: query layout from the live webview and print coordinates
import { McpClient } from './mcp-client.mjs';

const client = new McpClient(8089);
await client.connect();
console.log('Connected, session:', client.sessionId);

const script = `
  function rect(el) {
    if (!el) return null;
    const r = el.getBoundingClientRect();
    return { x: Math.round(r.x + r.width/2), y: Math.round(r.y + r.height/2), w: Math.round(r.width), h: Math.round(r.height) };
  }
  const wsItems = Array.from(document.querySelectorAll('.workspace-item')).map(el => ({
    ...rect(el),
    name: (el.querySelector('.workspace-name')?.textContent || '').trim(),
  }));
  const addWsBtn = rect(document.querySelector('.add-workspace-btn'));
  const tabs = Array.from(document.querySelectorAll('.tab')).map(el => ({
    ...rect(el),
    title: (el.querySelector('.tab-title')?.textContent || '').trim(),
  }));
  const addTabBtn = rect(document.querySelector('.add-tab-btn'));
  const terminalPane = rect(
    Array.from(document.querySelectorAll('.terminal-pane')).find(el => el.getBoundingClientRect().width > 0)
    || document.querySelector('.terminal-container')
  );
  return { workspaces: wsItems, addWsBtn, tabs, addTabBtn, terminalPane };
`;

const result = await client.callTool('execute_js', { script });
const text = result.content[0].text;
const data = JSON.parse(text);
const layout = JSON.parse(data.result);

console.log('\nViewport coordinates (relative to webview):');
console.log('  Workspaces:');
layout.workspaces.forEach((ws, i) => console.log(`    [${i}] "${ws.name}" → (${ws.x}, ${ws.y})  ${ws.w}x${ws.h}`));
console.log(`  Add workspace btn → (${layout.addWsBtn?.x}, ${layout.addWsBtn?.y})`);
console.log('  Tabs:');
layout.tabs.forEach((t, i) => console.log(`    [${i}] "${t.title}" → (${t.x}, ${t.y})  ${t.w}x${t.h}`));
console.log(`  Add tab btn → (${layout.addTabBtn?.x}, ${layout.addTabBtn?.y})`);
console.log(`  Terminal pane → (${layout.terminalPane?.x}, ${layout.terminalPane?.y})  ${layout.terminalPane?.w}x${layout.terminalPane?.h}`);

await client.close();
process.exit(0);
