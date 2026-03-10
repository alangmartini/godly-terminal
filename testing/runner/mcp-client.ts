import { spawn, ChildProcess } from 'child_process';
import { resolve as resolvePath } from 'path';

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
      // Log MCP stderr for debugging
      process.stderr.write(`[mcp] ${data}`);
    });

    // Send initialize request
    await this.call('initialize', {
      protocolVersion: '2024-11-05',
      capabilities: {},
      clientInfo: { name: 'godly-test-runner', version: '0.1.0' },
    });
  }

  async call(method: string, params: Record<string, unknown> = {}): Promise<unknown> {
    if (!this.process) throw new Error('Not connected');

    const id = ++this.requestId;
    const request = {
      jsonrpc: '2.0' as const,
      id,
      method,
      params,
    };

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      const msg = JSON.stringify(request);
      this.process!.stdin!.write(`Content-Length: ${Buffer.byteLength(msg)}\r\n\r\n${msg}`);
    });
  }

  async callTool(tool: string, args: Record<string, unknown> = {}): Promise<unknown> {
    const result = await this.call('tools/call', { name: tool, arguments: args });
    return result;
  }

  private handleData(chunk: string) {
    this.buffer += chunk;
    while (this.buffer.includes('\r\n\r\n')) {
      const headerEnd = this.buffer.indexOf('\r\n\r\n');
      const header = this.buffer.substring(0, headerEnd);
      const match = header.match(/Content-Length:\s*(\d+)/i);
      if (!match) {
        this.buffer = this.buffer.substring(headerEnd + 4);
        continue;
      }
      const length = parseInt(match[1], 10);
      const bodyStart = headerEnd + 4;
      if (this.buffer.length < bodyStart + length) break;

      const body = this.buffer.substring(bodyStart, bodyStart + length);
      this.buffer = this.buffer.substring(bodyStart + length);

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
        // Ignore parse errors
      }
    }
  }

  private findMcpBinary(): string {
    // Look for the staging MCP binary
    const candidates = [
      resolvePath(__dirname, '../../src-tauri/target/debug/godly-mcp.exe'),
      resolvePath(__dirname, '../../src-tauri/target/release/godly-mcp.exe'),
    ];
    // Return first candidate — actual existence check happens at spawn time
    return candidates[0];
  }

  async disconnect(): Promise<void> {
    if (this.process) {
      this.process.kill();
      this.process = null;
    }
    this.pending.clear();
  }
}
