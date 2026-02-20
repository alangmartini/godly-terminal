import { describe, it, expect, vi, beforeEach } from 'vitest';
import { PluginEventBus } from './event-bus';
import type { PluginEvent } from './types';

describe('PluginEventBus', () => {
  let bus: PluginEventBus;

  beforeEach(() => {
    bus = new PluginEventBus();
  });

  it('calls handlers registered for the emitted event type', () => {
    const handler = vi.fn();
    bus.on('notification', handler);

    bus.emit({ type: 'notification', timestamp: Date.now() });

    expect(handler).toHaveBeenCalledTimes(1);
    expect(handler).toHaveBeenCalledWith(expect.objectContaining({ type: 'notification' }));
  });

  it('does not call handlers for a different event type', () => {
    const handler = vi.fn();
    bus.on('agent:error', handler);

    bus.emit({ type: 'notification', timestamp: Date.now() });

    expect(handler).not.toHaveBeenCalled();
  });

  it('unsubscribes when the returned function is called', () => {
    const handler = vi.fn();
    const unsub = bus.on('notification', handler);

    unsub();
    bus.emit({ type: 'notification', timestamp: Date.now() });

    expect(handler).not.toHaveBeenCalled();
  });

  it('returns soundHandled=false when no plugin marks it', () => {
    bus.on('notification', () => {});
    const result = bus.emit({ type: 'notification', timestamp: Date.now() });

    expect(result.soundHandled).toBe(false);
  });

  it('returns soundHandled=true when a handler marks it', () => {
    bus.on('notification', () => {
      bus.markSoundHandled('notification');
    });

    const result = bus.emit({ type: 'notification', timestamp: Date.now() });

    expect(result.soundHandled).toBe(true);
  });

  it('catches handler errors without breaking other handlers', () => {
    const good = vi.fn();
    bus.on('notification', () => { throw new Error('kaboom'); });
    bus.on('notification', good);

    bus.emit({ type: 'notification', timestamp: Date.now() });

    expect(good).toHaveBeenCalledTimes(1);
  });

  it('classifies error messages into agent:error', () => {
    const handler = vi.fn();
    bus.on('agent:error', handler);

    bus.emitMcpNotify('t1', 'Build failed with errors');

    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('classifies permission messages into agent:permission', () => {
    const handler = vi.fn();
    bus.on('agent:permission', handler);

    bus.emitMcpNotify('t1', 'Please approve this action');

    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('classifies completion messages into agent:task-complete', () => {
    const handler = vi.fn();
    bus.on('agent:task-complete', handler);

    bus.emitMcpNotify('t1', 'Task completed successfully');

    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('falls back to notification for unclassified messages', () => {
    const handler = vi.fn();
    bus.on('notification', handler);

    bus.emitMcpNotify('t1', 'Something happened');

    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('falls back to notification for null messages', () => {
    const handler = vi.fn();
    bus.on('notification', handler);

    bus.emitMcpNotify('t1', null);

    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('emits both classified and notification events for classified messages', () => {
    const errorHandler = vi.fn();
    const notifHandler = vi.fn();
    bus.on('agent:error', errorHandler);
    bus.on('notification', notifHandler);

    bus.emitMcpNotify('t1', 'Build failed');

    expect(errorHandler).toHaveBeenCalledTimes(1);
    // Also emits a generic notification
    expect(notifHandler).toHaveBeenCalledTimes(1);
  });

  it('removeAllHandlers clears all subscriptions', () => {
    const handler = vi.fn();
    bus.on('notification', handler);
    bus.on('agent:error', handler);

    bus.removeAllHandlers();
    bus.emit({ type: 'notification', timestamp: Date.now() });
    bus.emit({ type: 'agent:error', timestamp: Date.now() });

    expect(handler).not.toHaveBeenCalled();
  });
});
