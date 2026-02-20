import type { PluginEvent, PluginEventType } from './types';

type EventHandler = (event: PluginEvent) => void;

// Keyword heuristics for classifying mcp-notify messages
const ERROR_KEYWORDS = [
  'error', 'fail', 'crash', 'exception', 'panic',
  'fatal', 'abort', 'denied', 'refused', 'timeout',
];

const PERMISSION_KEYWORDS = [
  'permission', 'approve', 'confirm', 'allow', 'accept',
  'authorize', 'consent', 'grant',
];

const COMPLETE_KEYWORDS = [
  'complete', 'done', 'finish', 'success', 'passed',
  'ready', 'built', 'compiled', 'deployed', 'merged',
];

function classifyMessage(message: string | null): PluginEventType {
  if (!message) return 'notification';
  const lower = message.toLowerCase();

  if (ERROR_KEYWORDS.some(kw => lower.includes(kw))) return 'agent:error';
  if (PERMISSION_KEYWORDS.some(kw => lower.includes(kw))) return 'agent:permission';
  if (COMPLETE_KEYWORDS.some(kw => lower.includes(kw))) return 'agent:task-complete';
  return 'notification';
}

export interface EmitResult {
  soundHandled: boolean;
}

export class PluginEventBus {
  private handlers = new Map<PluginEventType, Set<EventHandler>>();
  private soundHandledFlags = new Map<PluginEventType, boolean>();

  on(type: PluginEventType, handler: EventHandler): () => void {
    let set = this.handlers.get(type);
    if (!set) {
      set = new Set();
      this.handlers.set(type, set);
    }
    set.add(handler);
    return () => { set!.delete(handler); };
  }

  /** Mark that a plugin handled the sound for this event type */
  markSoundHandled(type: PluginEventType): void {
    this.soundHandledFlags.set(type, true);
  }

  emit(event: PluginEvent): EmitResult {
    this.soundHandledFlags.delete(event.type);

    const handlers = this.handlers.get(event.type);
    if (handlers) {
      for (const handler of handlers) {
        try {
          handler(event);
        } catch (e) {
          console.warn(`[PluginEventBus] Handler error for ${event.type}:`, e);
        }
      }
    }

    return { soundHandled: this.soundHandledFlags.get(event.type) ?? false };
  }

  /**
   * Classify an mcp-notify message and emit the appropriate event.
   * Returns the emit result so callers can check if sound was handled.
   */
  emitMcpNotify(terminalId: string, message: string | null): EmitResult {
    const classifiedType = classifyMessage(message);

    // First emit the classified event
    const classifiedResult = this.emit({
      type: classifiedType,
      terminalId,
      message: message ?? undefined,
      timestamp: Date.now(),
    });

    // If the classified type is different from 'notification', also emit
    // a generic 'notification' event (but only for non-sound purposes)
    if (classifiedType !== 'notification') {
      this.emit({
        type: 'notification',
        terminalId,
        message: message ?? undefined,
        timestamp: Date.now(),
      });
    }

    return classifiedResult;
  }

  removeAllHandlers(): void {
    this.handlers.clear();
  }
}
