// lib/step-executor.mjs
// Maps YAML step names to MCP tool calls, resolves variables, captures results.

import { parseToolResult } from './mcp-client.mjs';
import { runAssertion } from './assertions.mjs';

// camelCase YAML step → snake_case MCP tool name
const STEP_TO_TOOL = {
  resetApp:            'reset_staging_profile',
  waitForReady:        'wait_for_app_ready',
  createWorkspace:     'create_workspace',
  switchWorkspace:     'switch_workspace',
  deleteWorkspace:     'delete_workspace',
  renameWorkspace:     'rename_workspace',
  createTerminal:      'create_terminal',
  closeTerminal:       'close_terminal',
  renameTerminal:      'rename_terminal',
  focusTerminal:       'focus_terminal',
  executeCommand:      'execute_command',
  writeToTerminal:     'write_to_terminal',
  sendKeys:            'send_keys',
  readTerminal:        'read_terminal',
  readGrid:            'read_grid',
  waitForText:         'wait_for_text',
  waitForIdle:         'wait_for_idle',
  splitTerminal:       'split_terminal',
  createSplit:         'create_split',
  clearSplit:          'clear_split',
  unsplitTerminal:     'unsplit_terminal',
  screenshot:          'capture_screenshot',
  captureScreenshot:   'capture_screenshot',
  listWorkspaces:      'list_workspaces',
  listTerminals:       'list_terminals',
  getActiveWorkspace:  'get_active_workspace',
  getActiveTerminal:   'get_active_terminal',
  setTheme:            'set_theme',
  listThemes:          'list_themes',
  getActiveTheme:      'get_active_theme',
  eraseContent:        'erase_content',
  nextTab:             'next_tab',
  previousTab:         'previous_tab',
  goToTab:             'go_to_tab',
  getSplitState:       'get_split_state',
  getLayoutTree:       'get_layout_tree',
  swapPanes:           'swap_panes',
  zoomPane:            'zoom_pane',
  focusPane:           'focus_pane',
  focusOtherPane:      'focus_other_pane',
  resizePane:          'resize_pane',
  setSplitRatio:       'set_split_ratio',
  rotateSplit:         'rotate_split',
  scrollPageUp:        'scroll_page_up',
  scrollPageDown:      'scroll_page_down',
  scrollToTop:         'scroll_to_top',
  scrollToBottom:      'scroll_to_bottom',
  getAppInfo:          'get_app_info',
  copyToClipboard:     'copy_to_clipboard',
  getSelectedText:     'get_selected_text',
  exportStateDump:     'export_state_dump',
  collectArtifactBundle: 'collect_artifact_bundle',
  notify:              'notify',
};

// Steps that are built-in (not MCP tools)
const BUILT_IN_STEPS = new Set(['sleep', 'log', 'store']);

// Steps that are assertions
const ASSERTION_STEPS = new Set([
  'assertTextContains',
  'assertGridContains',
  'assertWorkspaceCount',
  'assertTerminalCount',
  'assertActiveWorkspace',
  'assertEqual',
  'assertNotEmpty',
]);

export class StepExecutor {
  constructor(mcpClient, cleanup) {
    this.mcp = mcpClient;
    this.cleanup = cleanup;
    this.vars = {};
  }

  /**
   * Execute a single normalized step.
   * Returns { success, result?, error?, duration }
   */
  async execute(step) {
    const start = performance.now();

    try {
      const resolvedArgs = resolveArgs(step.args, this.vars);

      let result;

      if (BUILT_IN_STEPS.has(step.action)) {
        result = await this._executeBuiltIn(step.action, resolvedArgs);
      } else if (ASSERTION_STEPS.has(step.action)) {
        result = await runAssertion(step.action, resolvedArgs, this.mcp, this.vars);
      } else if (STEP_TO_TOOL[step.action]) {
        const toolName = STEP_TO_TOOL[step.action];
        const rawResult = await this.mcp.callTool(toolName, resolvedArgs, {
          timeout: step.timeout || 60000,
        });
        result = parseToolResult(rawResult);

        // Track resources for cleanup
        this._trackResource(step.action, result);
      } else {
        // Try as raw snake_case tool name
        const rawResult = await this.mcp.callTool(step.action, resolvedArgs, {
          timeout: step.timeout || 60000,
        });
        result = parseToolResult(rawResult);
      }

      // Store result if requested
      if (step.store) {
        this.vars[step.store] = result;
      }

      const duration = performance.now() - start;
      return { success: true, result, duration };
    } catch (err) {
      const duration = performance.now() - start;
      return { success: false, error: err, duration };
    }
  }

  async _executeBuiltIn(action, args) {
    switch (action) {
      case 'sleep': {
        const ms = args.value || args.ms || 1000;
        await new Promise(r => setTimeout(r, ms));
        return { slept: ms };
      }
      case 'log': {
        const msg = args.value || args.message || '';
        console.log(`  [log] ${msg}`);
        return { logged: msg };
      }
      case 'store': {
        // Directly store a value: { store: { key: "val" } }
        return args;
      }
      default:
        throw new Error(`Unknown built-in step: ${action}`);
    }
  }

  _trackResource(action, result) {
    if (!this.cleanup) return;

    if (action === 'createWorkspace' && result?.workspace_id) {
      this.cleanup.trackWorkspace(result.workspace_id);
    }
    if (action === 'createTerminal' && result?.terminal_id) {
      this.cleanup.trackTerminal(result.terminal_id);
    }
  }

  /** Get a step's display label for the reporter */
  static getStepLabel(step) {
    const name = step.action;

    // Add key arg info for readability
    if (step.args.name) return `${name} "${step.args.name}"`;
    if (step.args.command) return `${name} "${truncate(step.args.command, 30)}"`;
    if (step.args.text) return `${name} "${truncate(step.args.text, 30)}"`;
    if (step.args.theme_name) return `${name} "${step.args.theme_name}"`;
    if (step.args.value !== undefined) return `${name} ${step.args.value}`;
    if (step.args.ms) return `${name} ${step.args.ms}ms`;

    return name;
  }
}

/**
 * Resolve $var.field references in args against stored variables.
 */
function resolveArgs(obj, vars) {
  if (typeof obj === 'string') {
    if (obj.startsWith('$')) {
      const ref = obj.slice(1);
      const parts = ref.split('.');
      let val = vars;
      for (const p of parts) {
        if (val == null) return obj;
        val = val[p];
      }
      return val ?? obj;
    }
    return obj;
  }
  if (Array.isArray(obj)) {
    return obj.map(item => resolveArgs(item, vars));
  }
  if (obj && typeof obj === 'object') {
    const result = {};
    for (const [k, v] of Object.entries(obj)) {
      result[k] = resolveArgs(v, vars);
    }
    return result;
  }
  return obj;
}

function truncate(str, max) {
  if (str.length <= max) return str;
  return str.slice(0, max - 1) + '\u2026';
}
