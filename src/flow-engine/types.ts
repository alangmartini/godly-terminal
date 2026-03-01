// ── Core Flow Types ──────────────────────────────────────────────────

export interface Flow {
  id: string;
  name: string;
  description: string;
  version: number;
  schemaVersion: 1;
  tags: string[];
  enabled: boolean;
  nodes: FlowNode[];
  edges: FlowEdge[];
  variables: FlowVariable[];
  createdAt: number;
  updatedAt: number;
  author?: string;
}

export interface FlowNode {
  id: string;
  type: string;
  label: string;
  position: { x: number; y: number };
  config: Record<string, unknown>;
  disabled: boolean;
}

export interface FlowEdge {
  id: string;
  sourceNodeId: string;
  sourcePort: string;
  targetNodeId: string;
  targetPort: string;
}

export interface FlowVariable {
  name: string;
  type: 'string' | 'number' | 'boolean';
  defaultValue: unknown;
}

// ── Port System ─────────────────────────────────────────────────────

export type PortValueType =
  | 'string'
  | 'number'
  | 'boolean'
  | 'terminal-id'
  | 'workspace-id'
  | 'any'
  | 'void';

export interface PortDefinition {
  name: string;
  label: string;
  direction: 'input' | 'output';
  valueType: PortValueType;
  required: boolean;
  defaultValue?: unknown;
}

// ── Node Type System ────────────────────────────────────────────────

export type NodeCategory =
  | 'trigger'
  | 'terminal'
  | 'split'
  | 'workspace'
  | 'voice'
  | 'quick-claude'
  | 'control-flow'
  | 'data';

export interface ConfigField {
  name: string;
  label: string;
  type: 'string' | 'number' | 'boolean' | 'select' | 'keychord';
  required: boolean;
  defaultValue?: unknown;
  options?: { label: string; value: string }[];
  placeholder?: string;
}

export interface NodeExecutionContext {
  flowId: string;
  runId: string;
  variables: Map<string, unknown>;
  abortSignal: AbortSignal;
}

export interface NodeTypeDefinition {
  type: string;
  category: NodeCategory;
  label: string;
  description: string;
  icon: string;
  ports: PortDefinition[];
  configSchema: ConfigField[];
  execute: (
    inputs: Record<string, unknown>,
    config: Record<string, unknown>,
    context: NodeExecutionContext,
  ) => Promise<Record<string, unknown>>;
}

// ── Execution Types ─────────────────────────────────────────────────

export type NodeRunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'skipped';

export interface NodeRunState {
  nodeId: string;
  status: NodeRunStatus;
  outputs: Record<string, unknown>;
  error?: string;
  startedAt?: number;
  completedAt?: number;
}

export type FlowRunStatus = 'pending' | 'running' | 'completed' | 'failed' | 'cancelled';

export interface FlowRun {
  id: string;
  flowId: string;
  status: FlowRunStatus;
  nodeStates: Map<string, NodeRunState>;
  startedAt: number;
  completedAt?: number;
  error?: string;
}

// ── Category Colors ─────────────────────────────────────────────────

export const NODE_CATEGORY_COLORS: Record<NodeCategory, string> = {
  'trigger': '#9ece6a',
  'terminal': '#7aa2f7',
  'split': '#7dcfff',
  'workspace': '#bb9af7',
  'voice': '#e0af68',
  'quick-claude': '#ff9e64',
  'control-flow': '#c0caf5',
  'data': '#89b4fa',
};

export const NODE_CATEGORY_LABELS: Record<NodeCategory, string> = {
  'trigger': 'Triggers',
  'terminal': 'Terminal',
  'split': 'Split / Layout',
  'workspace': 'Workspace',
  'voice': 'Voice',
  'quick-claude': 'Quick Claude',
  'control-flow': 'Control Flow',
  'data': 'Data',
};
