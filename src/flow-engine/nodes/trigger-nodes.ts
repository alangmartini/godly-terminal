import type { NodeTypeDefinition } from '../types';

// ── Trigger Nodes ──────────────────────────────────────────────────
//
// Triggers have no input ports and start a flow execution.

export const triggerNodes: NodeTypeDefinition[] = [
  // trigger.hotkey — fires when a configured key chord is pressed
  {
    type: 'trigger.hotkey',
    category: 'trigger',
    label: 'Hotkey Trigger',
    description: 'Fires when a keyboard shortcut is pressed.',
    icon: 'keyboard',
    ports: [
      {
        name: 'flow',
        label: 'Flow',
        direction: 'output',
        valueType: 'void',
        required: false,
      },
    ],
    configSchema: [
      {
        name: 'chord',
        label: 'Key Chord',
        type: 'keychord',
        required: true,
      },
    ],
    execute: async () => {
      // No-op — trigger nodes just start the flow, they don't produce data.
      return {};
    },
  },

  // trigger.manual — triggered via UI "Run" button, no configuration
  {
    type: 'trigger.manual',
    category: 'trigger',
    label: 'Manual Trigger',
    description: 'Triggered manually via the Run button in the flow editor.',
    icon: 'play',
    ports: [
      {
        name: 'flow',
        label: 'Flow',
        direction: 'output',
        valueType: 'void',
        required: false,
      },
    ],
    configSchema: [],
    execute: async () => {
      // No-op — manual triggers just start the flow.
      return {};
    },
  },
];
