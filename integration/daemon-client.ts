/**
 * TypeScript daemon client that speaks the godly-daemon wire protocol
 * over Windows named pipes.
 *
 * Wire protocol (from protocol/src/frame.rs):
 *   Frame: [4-byte BE u32 length][payload]
 *   Payload first byte:
 *     0x7B ('{') → JSON DaemonMessage
 *     0x01       → Binary Event::Output  [tag][sid_len:u8][sid][data]
 *     0x02       → Binary Request::Write [tag][sid_len:u8][sid][data]
 *     0x03       → Binary Response::Buffer [tag][sid_len:u8][sid][data]
 */

import net from 'node:net';
import { EventEmitter } from 'node:events';
import type { Request, Response, Event, DaemonMessage } from './protocol.js';
import { TAG_EVENT_OUTPUT, TAG_REQUEST_WRITE, TAG_RESPONSE_BUFFER } from './protocol.js';

const DEFAULT_TIMEOUT = 10_000;

interface PendingRequest {
  resolve: (resp: Response) => void;
  reject: (err: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

export interface DaemonClientOptions {
  /** Default timeout for requests in ms. Default: 10000 */
  timeout?: number;
}

export class DaemonClient extends EventEmitter {
  private socket: net.Socket | null = null;
  private recvBuffer = Buffer.alloc(0);
  private pendingQueue: PendingRequest[] = [];
  private defaultTimeout: number;
  private _connected = false;

  constructor(options?: DaemonClientOptions) {
    super();
    this.defaultTimeout = options?.timeout ?? DEFAULT_TIMEOUT;
  }

  get connected(): boolean {
    return this._connected;
  }

  /**
   * Connect to a named pipe with retry (daemon may still be starting).
   */
  async connect(pipeName: string, retries = 30, retryDelayMs = 200): Promise<void> {
    for (let attempt = 0; attempt < retries; attempt++) {
      try {
        await this._connectOnce(pipeName);
        return;
      } catch {
        if (attempt === retries - 1) {
          throw new Error(`Failed to connect to ${pipeName} after ${retries} attempts`);
        }
        await sleep(retryDelayMs);
      }
    }
  }

  private _connectOnce(pipeName: string): Promise<void> {
    return new Promise((resolve, reject) => {
      const socket = net.connect(pipeName);
      const onError = (err: Error) => {
        socket.removeListener('connect', onConnect);
        reject(err);
      };
      const onConnect = () => {
        socket.removeListener('error', onError);
        this.socket = socket;
        this._connected = true;
        this._setupSocket();
        resolve();
      };
      socket.once('connect', onConnect);
      socket.once('error', onError);
    });
  }

  private _setupSocket(): void {
    const socket = this.socket!;

    socket.on('data', (chunk: Buffer) => {
      this.recvBuffer = Buffer.concat([this.recvBuffer, chunk]);
      this._drainFrames();
    });

    socket.on('close', () => {
      this._connected = false;
      // Reject all pending requests
      for (const pending of this.pendingQueue) {
        clearTimeout(pending.timer);
        pending.reject(new Error('Connection closed'));
      }
      this.pendingQueue = [];
      this.emit('close');
    });

    socket.on('error', (err) => {
      this.emit('error', err);
    });
  }

  private _drainFrames(): void {
    while (this.recvBuffer.length >= 4) {
      const frameLen = this.recvBuffer.readUInt32BE(0);

      // Sanity check: reject frames > 16 MB (matches Rust limit)
      if (frameLen > 16 * 1024 * 1024) {
        this.emit('error', new Error(`Frame too large: ${frameLen} bytes`));
        this.disconnect();
        return;
      }

      if (this.recvBuffer.length < 4 + frameLen) {
        break; // Incomplete frame, wait for more data
      }

      const payload = this.recvBuffer.subarray(4, 4 + frameLen);
      this.recvBuffer = this.recvBuffer.subarray(4 + frameLen);

      this._handleFrame(payload);
    }
  }

  private _handleFrame(payload: Buffer): void {
    if (payload.length === 0) {
      this.emit('error', new Error('Empty frame'));
      return;
    }

    const firstByte = payload[0];

    if (firstByte === 0x7B) {
      // JSON DaemonMessage
      const msg: DaemonMessage = JSON.parse(payload.toString('utf-8'));
      if (msg.kind === 'Response') {
        this._deliverResponse(msg as Response);
      } else if (msg.kind === 'Event') {
        this.emit('event', msg as Event);
      }
    } else if (firstByte === TAG_EVENT_OUTPUT) {
      // Binary Event::Output
      const { sessionId, data } = decodeBinaryFrame(payload);
      const event: Event = { type: 'Output', session_id: sessionId, data: Array.from(data) };
      this.emit('event', event);
    } else if (firstByte === TAG_RESPONSE_BUFFER) {
      // Binary Response::Buffer
      const { sessionId, data } = decodeBinaryFrame(payload);
      const resp: Response = { type: 'Buffer', session_id: sessionId, data: Array.from(data) };
      this._deliverResponse(resp);
    } else {
      this.emit('error', new Error(`Unknown frame tag: 0x${firstByte.toString(16)}`));
    }
  }

  private _deliverResponse(resp: Response): void {
    const pending = this.pendingQueue.shift();
    if (pending) {
      clearTimeout(pending.timer);
      pending.resolve(resp);
    } else {
      this.emit('error', new Error(`Unexpected response with no pending request: ${resp.type}`));
    }
  }

  /**
   * Send a request and wait for the corresponding response.
   */
  async sendRequest(request: Request, timeoutMs?: number): Promise<Response> {
    if (!this.socket || !this._connected) {
      throw new Error('Not connected');
    }

    const timeout = timeoutMs ?? this.defaultTimeout;

    // Use binary framing for Write requests
    if (request.type === 'Write') {
      const frame = encodeBinaryFrame(TAG_REQUEST_WRITE, request.session_id, Buffer.from(request.data));
      this._sendFrame(frame);
    } else {
      const json = Buffer.from(JSON.stringify(request), 'utf-8');
      this._sendFrame(json);
    }

    return new Promise<Response>((resolve, reject) => {
      const timer = setTimeout(() => {
        const idx = this.pendingQueue.findIndex((p) => p.resolve === resolve);
        if (idx !== -1) this.pendingQueue.splice(idx, 1);
        reject(new Error(`Request timed out after ${timeout}ms: ${request.type}`));
      }, timeout);

      this.pendingQueue.push({ resolve, reject, timer });
    });
  }

  private _sendFrame(payload: Buffer): void {
    const header = Buffer.alloc(4);
    header.writeUInt32BE(payload.length, 0);
    this.socket!.write(Buffer.concat([header, payload]));
  }

  /**
   * Disconnect from the daemon.
   */
  disconnect(): void {
    if (this.socket) {
      this.socket.destroy();
      this.socket = null;
      this._connected = false;
    }
  }
}

// ── Binary frame helpers ─────────────────────────────────────────────────

function encodeBinaryFrame(tag: number, sessionId: string, data: Buffer): Buffer {
  const sidBuf = Buffer.from(sessionId, 'utf-8');
  const buf = Buffer.alloc(2 + sidBuf.length + data.length);
  buf[0] = tag;
  buf[1] = sidBuf.length;
  sidBuf.copy(buf, 2);
  data.copy(buf, 2 + sidBuf.length);
  return buf;
}

function decodeBinaryFrame(payload: Buffer): { sessionId: string; data: Buffer } {
  const sidLen = payload[1];
  const sessionId = payload.subarray(2, 2 + sidLen).toString('utf-8');
  const data = payload.subarray(2 + sidLen);
  return { sessionId, data };
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
