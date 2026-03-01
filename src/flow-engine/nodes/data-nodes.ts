import type { NodeTypeDefinition } from '../types';

// ── Data Nodes ─────────────────────────────────────────────────────
//
// Data manipulation and transformation nodes.

export const dataNodes: NodeTypeDefinition[] = [
  // data.constant — emits a fixed value
  {
    type: 'data.constant',
    category: 'data',
    label: 'Constant',
    description: 'Emits a fixed constant value (string, number, or boolean).',
    icon: 'hash',
    ports: [
      {
        name: 'value',
        label: 'Value',
        direction: 'output',
        valueType: 'any',
        required: false,
      },
    ],
    configSchema: [
      {
        name: 'value',
        label: 'Value',
        type: 'string',
        required: true,
        placeholder: 'Enter a value...',
      },
      {
        name: 'valueType',
        label: 'Type',
        type: 'select',
        required: true,
        defaultValue: 'string',
        options: [
          { label: 'String', value: 'string' },
          { label: 'Number', value: 'number' },
          { label: 'Boolean', value: 'boolean' },
        ],
      },
    ],
    execute: async (_inputs, config) => {
      const rawValue = config.value as string;
      const valueType = (config.valueType as string) ?? 'string';

      let value: unknown = rawValue;
      if (valueType === 'number') {
        value = Number(rawValue);
      } else if (valueType === 'boolean') {
        value = rawValue === 'true' || rawValue === '1';
      }

      return { value };
    },
  },

  // data.template — string interpolation with {{portName}} placeholders
  {
    type: 'data.template',
    category: 'data',
    label: 'Template',
    description:
      'String interpolation. Replaces {{portName}} placeholders with input values.',
    icon: 'code',
    ports: [
      {
        name: 'result',
        label: 'Result',
        direction: 'output',
        valueType: 'string',
        required: false,
      },
    ],
    configSchema: [
      {
        name: 'template',
        label: 'Template',
        type: 'string',
        required: true,
        placeholder: 'Hello, {{name}}! You have {{count}} items.',
      },
    ],
    execute: async (inputs, config) => {
      const template = (config.template as string) ?? '';

      // Replace all {{key}} placeholders with corresponding input values
      const result = template.replace(
        /\{\{(\w+)\}\}/g,
        (_match, key: string) => {
          const val = inputs[key];
          return val !== undefined && val !== null ? String(val) : '';
        },
      );

      return { result };
    },
  },
];
