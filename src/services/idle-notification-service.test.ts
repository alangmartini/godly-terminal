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

  function createService(opts?: {
    idleThresholdMs?: number;
    checkIntervalMs?: number;
    startupGraceMs?: number;
    notifyCooldownMs?: number;
  }) {
    service = new IdleNotificationService({
      idleThresholdMs: opts?.idleThresholdMs ?? 1000,
      checkIntervalMs: opts?.checkIntervalMs ?? 500,
      startupGraceMs: opts?.startupGraceMs,
      notifyCooldownMs: opts?.notifyCooldownMs,
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

  // ── Bug #209: Notification storm on startup ────────────────────────────
  //
  // When Godly Terminal starts, it reattaches to live daemon sessions. The
  // ring buffer replay generates terminal-output events for ALL terminals.
  // IdleNotificationService.recordOutput() is called for each one, and after
  // the idle threshold (15s), ALL non-active background terminals fire
  // notifications simultaneously — causing a notification storm.
  //
  // The service needs a startup suppression window so that output arriving
  // during reconnection does not trigger idle notifications.

  describe('Bug #209: startup notification storm', () => {
    it('should NOT notify for all terminals when bulk output arrives during startup (simulating reconnection replay)', () => {
      // Bug #209: On startup, ring buffer replay calls recordOutput() for all
      // terminals near-simultaneously. After idle threshold, all fire notifications.
      // Fix: startupGraceMs suppresses output recorded during the grace window.
      createService({ idleThresholdMs: 1000, checkIntervalMs: 500, startupGraceMs: 10000 });

      // Simulate startup: 5 terminals all receive output at the same time
      // (ring buffer replay during reattach)
      service.recordOutput('term-1');
      service.recordOutput('term-2');
      service.recordOutput('term-3');
      service.recordOutput('term-4');
      service.recordOutput('term-5');

      // Only term-1 is active (focused)
      activeTerminalId = 'term-1';

      // Wait past idle threshold
      vi.advanceTimersByTime(1500);

      // FIXED: 0 notifications — startup grace suppresses replay output.
      expect(onNotify).not.toHaveBeenCalled();
    });

    it('should suppress notifications during a startup grace period even if only one background terminal replays', () => {
      // Bug #209: Even a single background terminal receiving replay output
      // should not trigger a notification during the startup window.
      createService({ idleThresholdMs: 1000, checkIntervalMs: 500, startupGraceMs: 10000 });

      // Simulate: one background terminal gets replay output immediately after creation
      service.recordOutput('term-bg');
      activeTerminalId = 'term-active';

      // Wait past idle threshold
      vi.advanceTimersByTime(1500);

      // FIXED: No notification during startup grace period.
      expect(onNotify).not.toHaveBeenCalled();
    });

    it('should still notify for GENUINE output that arrives well after startup', () => {
      // After the startup grace period, normal idle detection should work.
      createService({ idleThresholdMs: 1000, checkIntervalMs: 500, startupGraceMs: 10000 });

      // Simulate startup replay (suppressed by grace)
      service.recordOutput('term-1');
      activeTerminalId = 'term-active';

      // Wait well past startup grace period (30s >> 10s grace)
      vi.advanceTimersByTime(30000);

      // Reset notification mock
      onNotify.mockClear();

      // Now genuine new output arrives on a background terminal
      service.recordOutput('term-1');

      // Wait past idle threshold
      vi.advanceTimersByTime(1500);

      // This SHOULD trigger a notification — it's genuine post-startup activity
      expect(onNotify).toHaveBeenCalledWith('term-1');
      expect(onNotify).toHaveBeenCalledTimes(1);
    });

    it('should not fire a notification storm for 10 background terminals on startup', () => {
      // Bug #209: Realistic scenario — user has 10 tabs open across workspaces,
      // restarts the app, and gets bombarded with 9 notifications.
      createService({ idleThresholdMs: 1000, checkIntervalMs: 500, startupGraceMs: 10000 });

      // Simulate startup: all 10 terminals get replay output
      for (let i = 0; i < 10; i++) {
        service.recordOutput(`term-${i}`);
      }

      // User is focused on term-0
      activeTerminalId = 'term-0';

      // Wait past idle threshold
      vi.advanceTimersByTime(2000);

      // FIXED: Zero notifications during startup grace.
      expect(onNotify).toHaveBeenCalledTimes(0);
    });
  });

  // ── Bug #209: Spurious idle notifications from background noise ────────
  //
  // Even when nothing meaningful is happening (Claude Code idle, shell
  // prompt sitting there), minor background terminal activity (cursor
  // repositioning, prompt redraws, periodic status updates) triggers
  // the output→silence→notification cycle repeatedly.
  //
  // The service should have a way to suppress repeated idle notifications
  // for terminals that haven't had substantial new output.

  describe('Bug #209: spurious idle notifications from background noise', () => {
    it('should NOT re-notify for trivially small output gaps (rapid idle cycling)', () => {
      // Bug #209: Background terminal produces tiny bursts of output
      // (e.g., cursor repositioning) with gaps just over the idle threshold.
      // Each gap triggers another notification.
      // Fix: notifyCooldownMs prevents re-notification within the cooldown period.
      createService({
        idleThresholdMs: 1000, checkIntervalMs: 500,
        startupGraceMs: 5000, notifyCooldownMs: 30000,
      });
      activeTerminalId = 'term-active';

      // Advance past startup grace so output is tracked normally
      vi.advanceTimersByTime(6000);

      // First burst — triggers initial notification (acceptable)
      service.recordOutput('term-bg');
      vi.advanceTimersByTime(1500);

      // Second tiny burst — just cursor activity, not meaningful
      service.recordOutput('term-bg');
      vi.advanceTimersByTime(1500);

      // Third tiny burst
      service.recordOutput('term-bg');
      vi.advanceTimersByTime(1500);

      // Fourth tiny burst
      service.recordOutput('term-bg');
      vi.advanceTimersByTime(1500);

      // FIXED: Only 1 notification — cooldown suppresses subsequent re-notifications.
      expect(onNotify).toHaveBeenCalledTimes(1);
    });

    it('should suppress notifications during the startup window for staggered reconnections', () => {
      // Bug #209: Terminals don't all reattach at exactly the same instant.
      // Some finish reconnection a few seconds after others. All should be
      // suppressed during the startup window.
      createService({ idleThresholdMs: 1000, checkIntervalMs: 500, startupGraceMs: 10000 });
      activeTerminalId = 'term-active';

      // First batch of terminals reconnect immediately
      service.recordOutput('term-1');
      service.recordOutput('term-2');

      // 3 seconds later, more terminals finish reconnecting (still within grace)
      vi.advanceTimersByTime(3000);
      service.recordOutput('term-3');
      service.recordOutput('term-4');

      // Wait for all idle thresholds to pass
      vi.advanceTimersByTime(2000);

      // FIXED: Zero notifications — all arrived during the startup grace window.
      expect(onNotify).not.toHaveBeenCalled();
    });
  });
});
