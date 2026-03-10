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
    const msg = JSON.stringify({ jsonrpc: '2.0', id, method, params });

    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      this.process!.stdin!.write(`Content-Length: ${Buffer.byteLength(msg)}\r\n\r\n${msg}`);
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
      // Content-Length is byte count, but buffer is a decoded string.
      // For ASCII-only JSON-RPC this is equivalent; for multi-byte content
      // we convert to bytes to check correctly.
      const bodyBytes = Buffer.byteLength(this.buffer.substring(bodyStart));
      if (bodyBytes < length) break;

      // Extract exactly `length` bytes worth of string characters
      const body = extractByteSlice(this.buffer, bodyStart, length);
      this.buffer = this.buffer.substring(bodyStart + body.length);

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

/** Extract a substring that is exactly `byteLen` bytes of UTF-8 from `str` starting at char index `start`. */
function extractByteSlice(str: string, start: number, byteLen: number): string {
  let bytes = 0;
  let i = start;
  while (i < str.length && bytes < byteLen) {
    const code = str.codePointAt(i)!;
    const charBytes = code <= 0x7f ? 1 : code <= 0x7ff ? 2 : code <= 0xffff ? 3 : 4;
    bytes += charBytes;
    i += code > 0xffff ? 2 : 1; // surrogate pair takes 2 JS chars
  }
  return str.substring(start, i);
}
