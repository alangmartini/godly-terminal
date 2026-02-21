/**
 * Detects when a terminal transitions from active output to silence while
 * not focused, and triggers a notification. This catches the scenario where
 * a CLI tool (e.g., Claude Code) is waiting for user input in a background tab.
 */

export interface IdleNotificationServiceOptions {
  /** How long (ms) a terminal must be silent after output to trigger notification. Default: 15000. */
  idleThresholdMs?: number;
  /** How often (ms) to check for idle terminals. Default: 5000. */
  checkIntervalMs?: number;
  /**
   * Suppress idle notifications for this many ms after service creation.
   * Prevents notification storms from ring buffer replay during reconnection.
   * Output recorded during this window is ignored (not tracked as recent activity).
   * Default: 0 (no suppression).
   */
  startupGraceMs?: number;
  /**
   * Minimum ms between consecutive notifications for the same terminal.
   * Prevents rapid idle-cycling from producing repeated spurious notifications.
   * Default: 0 (no cooldown).
   */
  notifyCooldownMs?: number;
  /** Returns the currently active (focused) terminal ID, or undefined if none. */
  getActiveTerminalId: () => string | undefined;
  /** Called when an idle notification should be shown for a terminal. */
  onNotify: (terminalId: string) => void;
}

interface TerminalTracker {
  lastOutputTime: number;
  hadRecentOutput: boolean;
  notified: boolean;
  /** Timestamp of the last notification fired for this terminal (0 = never). */
  lastNotifiedTime: number;
}

export class IdleNotificationService {
  private trackers = new Map<string, TerminalTracker>();
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private idleThresholdMs: number;
  private startupGraceMs: number;
  private notifyCooldownMs: number;
  private createdAt: number;
  private getActiveTerminalId: () => string | undefined;
  private onNotify: (terminalId: string) => void;

  constructor(options: IdleNotificationServiceOptions) {
    this.idleThresholdMs = options.idleThresholdMs ?? 15000;
    this.startupGraceMs = options.startupGraceMs ?? 0;
    this.notifyCooldownMs = options.notifyCooldownMs ?? 0;
    this.createdAt = Date.now();
    this.getActiveTerminalId = options.getActiveTerminalId;
    this.onNotify = options.onNotify;

    const checkInterval = options.checkIntervalMs ?? 5000;
    this.intervalId = setInterval(() => this.tick(), checkInterval);
  }

  /** Record that a terminal produced output. */
  recordOutput(terminalId: string): void {
    const now = Date.now();
    const inGrace = this.startupGraceMs > 0 && (now - this.createdAt) < this.startupGraceMs;

    const tracker = this.trackers.get(terminalId);
    if (tracker) {
      tracker.lastOutputTime = now;
      // During startup grace, don't mark output as recent activity —
      // it's likely ring buffer replay from reconnection, not new work.
      if (!inGrace) {
        tracker.hadRecentOutput = true;
        tracker.notified = false;
      }
    } else {
      this.trackers.set(terminalId, {
        lastOutputTime: now,
        hadRecentOutput: !inGrace,
        notified: false,
        lastNotifiedTime: 0,
      });
    }
  }

  /** Stop tracking a terminal (e.g., when it closes). */
  recordTerminalClosed(terminalId: string): void {
    this.trackers.delete(terminalId);
  }

  /** Periodic check: find terminals that went idle and notify. */
  private tick(): void {
    const now = Date.now();
    const activeId = this.getActiveTerminalId();

    for (const [terminalId, tracker] of this.trackers) {
      // Skip the currently focused terminal
      if (terminalId === activeId) continue;
      // Skip if no recent output or already notified
      if (!tracker.hadRecentOutput || tracker.notified) continue;

      const idleMs = now - tracker.lastOutputTime;
      if (idleMs >= this.idleThresholdMs) {
        // Check per-terminal cooldown: suppress repeated notifications
        // from rapid idle cycling (e.g., background cursor activity)
        if (this.notifyCooldownMs > 0 && tracker.lastNotifiedTime > 0 &&
            (now - tracker.lastNotifiedTime) < this.notifyCooldownMs) {
          tracker.hadRecentOutput = false;
          tracker.notified = true;
          continue;
        }

        tracker.hadRecentOutput = false;
        tracker.notified = true;
        tracker.lastNotifiedTime = now;
        this.onNotify(terminalId);
      }
    }
  }

  /** Clean up the interval timer. */
  destroy(): void {
    if (this.intervalId !== null) {
      clearInterval(this.intervalId);
      this.intervalId = null;
    }
  }
}
