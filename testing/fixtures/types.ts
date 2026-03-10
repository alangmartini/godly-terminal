export interface McpClient {
  call(tool: string, args?: Record<string, unknown>): Promise<unknown>;
}

export interface Fixture {
  name: string;
  description: string;
  create(client: McpClient): Promise<void>;
  verifyReady(client: McpClient): Promise<boolean>;
  tearDown(client: McpClient): Promise<void>;
  reset(client: McpClient): Promise<void>;
}
