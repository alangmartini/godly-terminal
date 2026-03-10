import { spawn, ChildProcess } from 'child_process';
import { resolve as resolvePath } from 'path';
import { existsSync } from 'fs';

export class McpClient {
  private process: ChildProcess | null = null;
  private requestId = 0;
  private pending = new Map<number, { resolve: (v: unknown) => void; reject: (e: Error) => void }>();
  private buffer = '';

  constructor(private mcpBinaryPath?: string) {}

  async connect(): Promise<void> {
    const binaryPath = this.mcpBinaryPath || this.findMcpBinary();

    this.process = spawn(binaryPath, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: {
        ...process.env,
        GODLY_INSTANCE: 'staging',
      },
    });

    this.process.stdout!.setEncoding('utf-8');
    this.process.stdout!.on('data', (data: string) => this.handleData(data));
    this.process.stderr!.on('data', (data: Buffer) => {
      process.stderr.write(`[mcp] ${data}`);
    });

    this.process.on('exit', (code) => {
      this.rejectAll(new Error(`MCP process exited with code ${code}`));
      this.process = null;
    });

    await this.call('initialize', {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'godly-test-runner', version: '0.1.0' },
    });
  }

  async call(method: string, params: Record<string, unknown> = {}): Promise<unknown> {
    if (!this.process) throw new Error('Not connected');

    const id = ++this.requestId;
    const msg = `${JSON.stringify({ jsonrpc: '2.0', id, method, params })}\n`;

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.process!.stdin!.write(msg);
    });
  }

  async callTool(tool: string, args: Record<string, unknown> = {}): Promise<unknown> {
    return this.call('tools/call', { name: tool, arguments: args });
  }

  async disconnect(): Promise<void> {
    if (this.process) {
      this.process.kill();
      this.process = null;
    }
    this.rejectAll(new Error('Client disconnected'));
  }

  private rejectAll(error: Error): void {
    for (const { reject } of this.pending.values()) reject(error);
    this.pending.clear();
  }

  private handleData(chunk: string) {
    this.buffer += chunk;
    while (true) {
      const newlineIndex = this.buffer.indexOf('\n');
      if (newlineIndex === -1) break;

      const body = this.buffer.substring(0, newlineIndex).trim();
      this.buffer = this.buffer.substring(newlineIndex + 1);
      if (!body) continue;

      try {
        const response = JSON.parse(body);
        if (response.id != null && this.pending.has(response.id)) {
          const { resolve, reject } = this.pending.get(response.id)!;
          this.pending.delete(response.id);
          if (response.error) {
            reject(new Error(response.error.message || JSON.stringify(response.error)));
          } else {
            resolve(response.result);
          }
        }
      } catch {
        // Malformed JSON -- skip this message
      }
    }
  }

  private findMcpBinary(): string {
    const candidates = [
      resolvePath(__dirname, '../../src-tauri/target/release/godly-mcp.exe'),
      resolvePath(__dirname, '../../src-tauri/target/debug/godly-mcp.exe'),
    ];
    for (const c of candidates) {
      if (existsSync(c)) return c;
    }
    // Fall back to first candidate; spawn will produce a clear error
    return candidates[0];
  }
}
