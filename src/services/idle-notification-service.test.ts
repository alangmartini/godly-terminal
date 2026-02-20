import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { IdleNotificationService } from './idle-notification-service';

describe('IdleNotificationService', () => {
  let service: IdleNotificationService;
  let onNotify: ReturnType<typeof vi.fn>;
  let activeTerminalId: string | undefined;

  beforeEach(() => {
    vi.useFakeTimers();
    onNotify = vi.fn();
    activeTerminalId = undefined;
  });

  afterEach(() => {
    service?.destroy();
    vi.useRealTimers();
  });

  function createService(opts?: { idleThresholdMs?: number; checkIntervalMs?: number }) {
    service = new IdleNotificationService({
      idleThresholdMs: opts?.idleThresholdMs ?? 1000,
      checkIntervalMs: opts?.checkIntervalMs ?? 500,
      getActiveTerminalId: () => activeTerminalId,
      onNotify,
    });
    return service;
  }

  it('notifies when a background terminal goes idle after output', () => {
    createService();
    service.recordOutput('term-1');

    // Advance past idle threshold + check interval
    vi.advanceTimersByTime(1500);

    expect(onNotify).toHaveBeenCalledWith('term-1');
    expect(onNotify).toHaveBeenCalledTimes(1);
  });

  it('does not notify for the active (focused) terminal', () => {
    createService();
    activeTerminalId = 'term-1';
    service.recordOutput('term-1');

    vi.advanceTimersByTime(2000);

    expect(onNotify).not.toHaveBeenCalled();
  });

  it('does not notify before idle threshold is reached', () => {
    createService({ idleThresholdMs: 5000 });
    service.recordOutput('term-1');

    // Only 2 seconds — not yet idle
    vi.advanceTimersByTime(2000);

    expect(onNotify).not.toHaveBeenCalled();
  });

  it('does not notify twice for the same idle period', () => {
    createService();
    service.recordOutput('term-1');

    // First tick triggers notification
    vi.advanceTimersByTime(1500);
    expect(onNotify).toHaveBeenCalledTimes(1);

    // Subsequent ticks should not re-notify
    vi.advanceTimersByTime(5000);
    expect(onNotify).toHaveBeenCalledTimes(1);
  });

  it('re-notifies after new output followed by another idle period', () => {
    createService();
    service.recordOutput('term-1');

    vi.advanceTimersByTime(1500);
    expect(onNotify).toHaveBeenCalledTimes(1);

    // New output resets the tracker
    service.recordOutput('term-1');
    vi.advanceTimersByTime(1500);
    expect(onNotify).toHaveBeenCalledTimes(2);
  });

  it('stops tracking a closed terminal', () => {
    createService();
    service.recordOutput('term-1');
    service.recordTerminalClosed('term-1');

    vi.advanceTimersByTime(2000);

    expect(onNotify).not.toHaveBeenCalled();
  });

  it('tracks multiple terminals independently', () => {
    createService();
    service.recordOutput('term-1');
    service.recordOutput('term-2');

    vi.advanceTimersByTime(1500);

    expect(onNotify).toHaveBeenCalledWith('term-1');
    expect(onNotify).toHaveBeenCalledWith('term-2');
    expect(onNotify).toHaveBeenCalledTimes(2);
  });

  it('skips focused terminal but notifies others', () => {
    createService();
    activeTerminalId = 'term-1';
    service.recordOutput('term-1');
    service.recordOutput('term-2');

    vi.advanceTimersByTime(1500);

    expect(onNotify).toHaveBeenCalledWith('term-2');
    expect(onNotify).toHaveBeenCalledTimes(1);
  });

  it('continuous output keeps resetting the idle timer', () => {
    createService({ idleThresholdMs: 1000, checkIntervalMs: 200 });
    service.recordOutput('term-1');

    // Keep producing output every 300ms — should never go idle
    for (let i = 0; i < 10; i++) {
      vi.advanceTimersByTime(300);
      service.recordOutput('term-1');
    }

    expect(onNotify).not.toHaveBeenCalled();
  });

  it('destroy stops the interval timer', () => {
    createService();
    service.recordOutput('term-1');
    service.destroy();

    vi.advanceTimersByTime(5000);

    expect(onNotify).not.toHaveBeenCalled();
  });
});
