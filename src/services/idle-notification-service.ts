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
  /** Returns the currently active (focused) terminal ID, or undefined if none. */
  getActiveTerminalId: () => string | undefined;
  /** Called when an idle notification should be shown for a terminal. */
  onNotify: (terminalId: string) => void;
}

interface TerminalTracker {
  lastOutputTime: number;
  hadRecentOutput: boolean;
  notified: boolean;
}

export class IdleNotificationService {
  private trackers = new Map<string, TerminalTracker>();
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private idleThresholdMs: number;
  private getActiveTerminalId: () => string | undefined;
  private onNotify: (terminalId: string) => void;

  constructor(options: IdleNotificationServiceOptions) {
    this.idleThresholdMs = options.idleThresholdMs ?? 15000;
    this.getActiveTerminalId = options.getActiveTerminalId;
    this.onNotify = options.onNotify;

    const checkInterval = options.checkIntervalMs ?? 5000;
    this.intervalId = setInterval(() => this.tick(), checkInterval);
  }

  /** Record that a terminal produced output. */
  recordOutput(terminalId: string): void {
    const now = Date.now();
    const tracker = this.trackers.get(terminalId);
    if (tracker) {
      tracker.lastOutputTime = now;
      tracker.hadRecentOutput = true;
      tracker.notified = false;
    } else {
      this.trackers.set(terminalId, {
        lastOutputTime: now,
        hadRecentOutput: true,
        notified: false,
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
        tracker.hadRecentOutput = false;
        tracker.notified = true;
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
