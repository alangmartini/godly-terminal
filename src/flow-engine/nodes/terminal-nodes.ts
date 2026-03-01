import type { ShellType } from '../../state/store';
import type { NodeTypeDefinition } from '../types';

// ── Terminal Nodes ─────────────────────────────────────────────────
//
// Terminal operation nodes wrapping existing Tauri invoke() calls and
// store methods.

export const terminalNodes: NodeTypeDefinition[] = [
  // terminal.create — creates a new terminal session
  {
    type: 'terminal.create',
    category: 'terminal',
    label: 'Create Terminal',
    description: 'Creates a new terminal in a workspace.',
    icon: 'plus-square',
    ports: [
      {
        name: 'workspaceId',
        label: 'Workspace ID',
        direction: 'input',
        valueType: 'workspace-id',
        required: true,
      },
      {
        name: 'terminalId',
        label: 'Terminal ID',
        direction: 'output',
        valueType: 'terminal-id',
        required: false,
      },
    ],
    configSchema: [
      {
        name: 'shellType',
        label: 'Shell Type',
        type: 'select',
        required: false,
        options: [
          { label: 'Default', value: '' },
          { label: 'PowerShell (Windows)', value: 'windows' },
          { label: 'PowerShell Core', value: 'pwsh' },
          { label: 'CMD', value: 'cmd' },
          { label: 'WSL', value: 'wsl' },
        ],
      },
    ],
    execute: async (inputs, config) => {
      const { terminalService } = await import('../../services/terminal-service');
      const workspaceId = inputs.workspaceId as string;

      // Map config shell type string to ShellType object
      let shellTypeOverride: ShellType | undefined;
      const shellValue = config.shellType as string | undefined;
      if (shellValue && shellValue !== '') {
        shellTypeOverride = { type: shellValue } as ShellType;
      }

      const result = await terminalService.createTerminal(workspaceId, {
        shellTypeOverride,
      });

      return { terminalId: result.id };
    },
  },

  // terminal.close — closes an existing terminal session
  {
    type: 'terminal.close',
    category: 'terminal',
    label: 'Close Terminal',
    description: 'Closes a terminal session.',
    icon: 'x-square',
    ports: [
      {
        name: 'terminalId',
        label: 'Terminal ID',
        direction: 'input',
        valueType: 'terminal-id',
        required: true,
      },
    ],
    configSchema: [],
    execute: async (inputs) => {
      const { terminalService } = await import('../../services/terminal-service');
      const terminalId = inputs.terminalId as string;
      await terminalService.closeTerminal(terminalId);
      return {};
    },
  },

  // terminal.write — writes raw data to a terminal
  {
    type: 'terminal.write',
    category: 'terminal',
    label: 'Write to Terminal',
    description: 'Writes raw data to a terminal (no newline appended).',
    icon: 'edit',
    ports: [
      {
        name: 'terminalId',
        label: 'Terminal ID',
        direction: 'input',
        valueType: 'terminal-id',
        required: true,
      },
      {
        name: 'data',
        label: 'Data',
        direction: 'input',
        valueType: 'string',
        required: true,
      },
    ],
    configSchema: [],
    execute: async (inputs) => {
      const { terminalService } = await import('../../services/terminal-service');
      const terminalId = inputs.terminalId as string;
      const data = inputs.data as string;
      await terminalService.writeToTerminal(terminalId, data);
      return {};
    },
  },

  // terminal.read — reads the current visible terminal grid text
  {
    type: 'terminal.read',
    category: 'terminal',
    label: 'Read Terminal',
    description: 'Reads the current visible terminal grid content as plain text.',
    icon: 'file-text',
    ports: [
      {
        name: 'terminalId',
        label: 'Terminal ID',
        direction: 'input',
        valueType: 'terminal-id',
        required: true,
      },
      {
        name: 'content',
        label: 'Content',
        direction: 'output',
        valueType: 'string',
        required: false,
      },
    ],
    configSchema: [],
    execute: async (inputs) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const terminalId = inputs.terminalId as string;

      // Get grid dimensions first
      const [rows, cols] = await invoke<[number, number]>('get_grid_dimensions', {
        terminalId,
      });

      // Read the full visible grid text (row 0 to last row, col 0 to last col)
      const text = await invoke<string>('get_grid_text', {
        terminalId,
        startRow: 0,
        startCol: 0,
        endRow: rows - 1,
        endCol: cols,
        scrollbackOffset: 0,
      });

      return { content: text };
    },
  },

  // terminal.execute — writes a command + carriage return (fire and forget)
  {
    type: 'terminal.execute',
    category: 'terminal',
    label: 'Execute Command',
    description: 'Writes a command followed by Enter to a terminal.',
    icon: 'terminal',
    ports: [
      {
        name: 'terminalId',
        label: 'Terminal ID',
        direction: 'input',
        valueType: 'terminal-id',
        required: true,
      },
      {
        name: 'command',
        label: 'Command',
        direction: 'input',
        valueType: 'string',
        required: true,
      },
    ],
    configSchema: [],
    execute: async (inputs) => {
      const { terminalService } = await import('../../services/terminal-service');
      const terminalId = inputs.terminalId as string;
      const command = inputs.command as string;
      await terminalService.writeToTerminal(terminalId, command + '\r');
      return {};
    },
  },

  // terminal.focus — focuses a terminal in the UI
  {
    type: 'terminal.focus',
    category: 'terminal',
    label: 'Focus Terminal',
    description: 'Sets a terminal as the active terminal in the UI.',
    icon: 'maximize',
    ports: [
      {
        name: 'terminalId',
        label: 'Terminal ID',
        direction: 'input',
        valueType: 'terminal-id',
        required: true,
      },
    ],
    configSchema: [],
    execute: async (inputs) => {
      const { store } = await import('../../state/store');
      const terminalId = inputs.terminalId as string;
      store.setActiveTerminal(terminalId);
      return {};
    },
  },

  // terminal.rename — renames a terminal tab
  {
    type: 'terminal.rename',
    category: 'terminal',
    label: 'Rename Terminal',
    description: 'Renames a terminal tab label.',
    icon: 'tag',
    ports: [
      {
        name: 'terminalId',
        label: 'Terminal ID',
        direction: 'input',
        valueType: 'terminal-id',
        required: true,
      },
      {
        name: 'name',
        label: 'Name',
        direction: 'input',
        valueType: 'string',
        required: true,
      },
    ],
    configSchema: [],
    execute: async (inputs) => {
      const { terminalService } = await import('../../services/terminal-service');
      const terminalId = inputs.terminalId as string;
      const name = inputs.name as string;
      await terminalService.renameTerminal(terminalId, name);
      return {};
    },
  },

  // terminal.waitForText — polls terminal grid until text appears or timeout
  {
    type: 'terminal.waitForText',
    category: 'terminal',
    label: 'Wait for Text',
    description: 'Polls the terminal grid until the specified text appears or timeout is reached.',
    icon: 'clock',
    ports: [
      {
        name: 'terminalId',
        label: 'Terminal ID',
        direction: 'input',
        valueType: 'terminal-id',
        required: true,
      },
      {
        name: 'text',
        label: 'Text to Find',
        direction: 'input',
        valueType: 'string',
        required: true,
      },
      {
        name: 'found',
        label: 'Found',
        direction: 'output',
        valueType: 'boolean',
        required: false,
      },
    ],
    configSchema: [
      {
        name: 'timeoutMs',
        label: 'Timeout (ms)',
        type: 'number',
        required: false,
        defaultValue: 10000,
        placeholder: '10000',
      },
      {
        name: 'pollIntervalMs',
        label: 'Poll Interval (ms)',
        type: 'number',
        required: false,
        defaultValue: 500,
        placeholder: '500',
      },
    ],
    execute: async (inputs, config, context) => {
      const { invoke } = await import('@tauri-apps/api/core');
      const terminalId = inputs.terminalId as string;
      const searchText = inputs.text as string;
      const timeoutMs = (config.timeoutMs as number) ?? 10000;
      const pollIntervalMs = (config.pollIntervalMs as number) ?? 500;

      const deadline = Date.now() + timeoutMs;

      while (Date.now() < deadline) {
        if (context.abortSignal.aborted) {
          return { found: false };
        }

        // Read full grid content
        const [rows, cols] = await invoke<[number, number]>('get_grid_dimensions', {
          terminalId,
        });
        const text = await invoke<string>('get_grid_text', {
          terminalId,
          startRow: 0,
          startCol: 0,
          endRow: rows - 1,
          endCol: cols,
          scrollbackOffset: 0,
        });

        if (text.includes(searchText)) {
          return { found: true };
        }

        // Wait for the poll interval, but abort early if signaled
        await new Promise<void>((resolve) => {
          const timer = setTimeout(resolve, pollIntervalMs);
          const onAbort = () => {
            clearTimeout(timer);
            resolve();
          };
          context.abortSignal.addEventListener('abort', onAbort, { once: true });
        });
      }

      return { found: false };
    },
  },
];
