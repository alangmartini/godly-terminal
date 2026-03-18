// lib/assertions.mjs
// Built-in assertion implementations that call MCP tools and check results.

import { parseToolResult } from './mcp-client.mjs';

/**
 * Run an assertion step.
 * Throws AssertionError on failure, returns result on success.
 */
export async function runAssertion(action, args, mcpClient, vars) {
  switch (action) {
    case 'assertTextContains':
      return await assertTextContains(args, mcpClient);
    case 'assertGridContains':
      return await assertGridContains(args, mcpClient);
    case 'assertWorkspaceCount':
      return await assertWorkspaceCount(args, mcpClient);
    case 'assertTerminalCount':
      return await assertTerminalCount(args, mcpClient);
    case 'assertActiveWorkspace':
      return await assertActiveWorkspace(args, mcpClient);
    case 'assertEqual':
      return await assertEqual(args, mcpClient, vars);
    case 'assertNotEmpty':
      return assertNotEmpty(args, vars);
    default:
      throw new Error(`Unknown assertion: ${action}`);
  }
}

/**
 * assertTextContains:
 *   terminal_id: "..."
 *   text: "expected substring"
 *   mode: "tail" (optional)
 *   lines: 100 (optional)
 */
async function assertTextContains(args, mcpClient) {
  const { terminal_id, text, mode, lines } = args;
  if (!terminal_id) throw new AssertionError('assertTextContains requires terminal_id');
  if (!text) throw new AssertionError('assertTextContains requires text');

  const raw = await mcpClient.callTool('read_terminal', {
    terminal_id,
    mode: mode || 'tail',
    lines: lines || 200,
    strip_ansi: true,
  });
  const result = parseToolResult(raw);
  const output = result.text || JSON.stringify(result);

  if (!output.includes(text)) {
    throw new AssertionError(
      `Expected terminal to contain "${text}"\n` +
      `  Terminal output (last lines):\n${indent(truncateLines(output, 10))}`
    );
  }

  return { matched: true, text };
}

/**
 * assertGridContains:
 *   terminal_id: "..."
 *   text: "expected substring"
 */
async function assertGridContains(args, mcpClient) {
  const { terminal_id, text } = args;
  if (!terminal_id) throw new AssertionError('assertGridContains requires terminal_id');
  if (!text) throw new AssertionError('assertGridContains requires text');

  const raw = await mcpClient.callTool('read_grid', { terminal_id });
  const result = parseToolResult(raw);
  const output = result.text || JSON.stringify(result);

  if (!output.includes(text)) {
    throw new AssertionError(
      `Expected grid to contain "${text}"\n` +
      `  Grid content:\n${indent(truncateLines(output, 10))}`
    );
  }

  return { matched: true, text };
}

/**
 * assertWorkspaceCount:
 *   count: N
 *   min: N (optional, alternative to exact count)
 */
async function assertWorkspaceCount(args, mcpClient) {
  const raw = await mcpClient.callTool('list_workspaces', {});
  const result = parseToolResult(raw);

  const workspaces = result.workspaces || result;
  const count = Array.isArray(workspaces) ? workspaces.length : 0;

  if (args.count !== undefined && count !== args.count) {
    throw new AssertionError(`Expected ${args.count} workspaces, got ${count}`);
  }
  if (args.min !== undefined && count < args.min) {
    throw new AssertionError(`Expected at least ${args.min} workspaces, got ${count}`);
  }

  return { count };
}

/**
 * assertTerminalCount:
 *   count: N
 *   min: N (optional)
 */
async function assertTerminalCount(args, mcpClient) {
  const raw = await mcpClient.callTool('list_terminals', {});
  const result = parseToolResult(raw);

  const terminals = result.terminals || result;
  const count = Array.isArray(terminals) ? terminals.length : 0;

  if (args.count !== undefined && count !== args.count) {
    throw new AssertionError(`Expected ${args.count} terminals, got ${count}`);
  }
  if (args.min !== undefined && count < args.min) {
    throw new AssertionError(`Expected at least ${args.min} terminals, got ${count}`);
  }

  return { count };
}

/**
 * assertActiveWorkspace:
 *   workspace_id: "expected-id"
 */
async function assertActiveWorkspace(args, mcpClient) {
  const { workspace_id } = args;
  if (!workspace_id) throw new AssertionError('assertActiveWorkspace requires workspace_id');

  const raw = await mcpClient.callTool('get_active_workspace', {});
  const result = parseToolResult(raw);

  const activeId = result.workspace_id || result.id;
  if (activeId !== workspace_id) {
    throw new AssertionError(`Expected active workspace "${workspace_id}", got "${activeId}"`);
  }

  return { workspace_id: activeId };
}

/**
 * assertEqual — generic assertion that calls any MCP tool and checks a field.
 *   tool: "tool_name"
 *   args: { ... }           (optional)
 *   field: "path.to.field"
 *   expected: value
 */
async function assertEqual(args, mcpClient, vars) {
  const { tool, field, expected } = args;
  const toolArgs = args.args || {};

  if (!tool) throw new AssertionError('assertEqual requires tool');
  if (!field) throw new AssertionError('assertEqual requires field');
  if (expected === undefined) throw new AssertionError('assertEqual requires expected');

  const raw = await mcpClient.callTool(tool, toolArgs);
  const result = parseToolResult(raw);

  // Navigate to field
  const parts = field.split('.');
  let actual = result;
  for (const p of parts) {
    if (actual == null) break;
    actual = actual[p];
  }

  if (actual !== expected) {
    throw new AssertionError(
      `assertEqual: ${field} — expected ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}`
    );
  }

  return { field, expected, actual };
}

/**
 * assertNotEmpty — check a stored variable is not null/undefined/empty.
 *   var: "varName"
 *   field: "optional.path" (optional)
 */
function assertNotEmpty(args, vars) {
  const varName = args.var;
  if (!varName) throw new AssertionError('assertNotEmpty requires var');

  let val = vars[varName];
  if (args.field) {
    for (const p of args.field.split('.')) {
      if (val == null) break;
      val = val[p];
    }
  }

  if (val === null || val === undefined || val === '' || (Array.isArray(val) && val.length === 0)) {
    const path = args.field ? `$${varName}.${args.field}` : `$${varName}`;
    throw new AssertionError(`Expected ${path} to not be empty, got ${JSON.stringify(val)}`);
  }

  return { value: val };
}

class AssertionError extends Error {
  constructor(message) {
    super(message);
    this.name = 'AssertionError';
  }
}

function indent(str, spaces = 4) {
  const pad = ' '.repeat(spaces);
  return str.split('\n').map(line => pad + line).join('\n');
}

function truncateLines(str, maxLines) {
  const lines = str.split('\n');
  if (lines.length <= maxLines) return str;
  return lines.slice(-maxLines).join('\n');
}
