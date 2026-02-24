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
  /**
   * Minimum number of output events a terminal must produce before an idle
   * notification can fire. Prevents false positives from minor terminal noise
   * (cursor updates, single-line status changes, prompt redraws) by requiring
   * a substantial burst of activity. Default: 1 (any output qualifies).
   */
  minOutputEvents?: number;
  /** Returns the currently active (focused) terminal ID, or undefined if none. */
  getActiveTerminalId: () => string | undefined;
  /** Called when an idle notification should be shown for a terminal. */
  onNotify: (terminalId: string) => void;
}

interface TerminalTracker {
  lastOutputTime: number;
  /** Number of output events since last notification (or creation). Replaces boolean hadRecentOutput. */
  outputEventCount: number;
  notified: boolean;
  /** Timestamp of the last notification fired for this terminal (0 = never). */
  lastNotifiedTime: number;
}

export class IdleNotificationService {
  private trackers = new Map<string, TerminalTracker>();
  /** Terminal IDs that have been closed — prevents zombie tracker re-creation. */
  private closedTerminals = new Set<string>();
  private intervalId: ReturnType<typeof setInterval> | null = null;
  private idleThresholdMs: number;
  private startupGraceMs: number;
  private notifyCooldownMs: number;
  private minOutputEvents: number;
  private createdAt: number;
  private getActiveTerminalId: () => string | undefined;
  private onNotify: (terminalId: string) => void;

  constructor(options: IdleNotificationServiceOptions) {
    this.idleThresholdMs = options.idleThresholdMs ?? 15000;
    this.startupGraceMs = options.startupGraceMs ?? 0;
    this.notifyCooldownMs = options.notifyCooldownMs ?? 0;
    this.minOutputEvents = options.minOutputEvents ?? 1;
    this.createdAt = Date.now();
    this.getActiveTerminalId = options.getActiveTerminalId;
    this.onNotify = options.onNotify;

    const checkInterval = options.checkIntervalMs ?? 5000;
    this.intervalId = setInterval(() => this.tick(), checkInterval);
  }

  /** Record that a terminal produced output. */
  recordOutput(terminalId: string): void {
    // Bug #272: Don't create trackers for closed terminals (prevents zombie trackers
    // from late output events arriving after terminal-closed).
    if (this.closedTerminals.has(terminalId)) return;

    const now = Date.now();
    const inGrace = this.startupGraceMs > 0 && (now - this.createdAt) < this.startupGraceMs;

    const tracker = this.trackers.get(terminalId);
    if (tracker) {
      tracker.lastOutputTime = now;
      // During startup grace, don't mark output as recent activity —
      // it's likely ring buffer replay from reconnection, not new work.
      if (!inGrace) {
        tracker.outputEventCount++;
        // Bug #272: Don't reset notified here. After a notification fires,
        // re-arming is handled by tick() only after extended true silence.
        // This prevents periodic shell noise from restarting the notification
        // cycle every time the cooldown expires.
      }
    } else {
      this.trackers.set(terminalId, {
        lastOutputTime: now,
        outputEventCount: inGrace ? 0 : 1,
        notified: false,
        lastNotifiedTime: 0,
      });
    }
  }

  /** Stop tracking a terminal (e.g., when it closes). */
  recordTerminalClosed(terminalId: string): void {
    this.trackers.delete(terminalId);
    // Bug #272: Remember this terminal was closed so late output events
    // don't silently re-create its tracker.
    this.closedTerminals.add(terminalId);
  }

  /** Periodic check: find terminals that went idle and notify. */
  private tick(): void {
    const now = Date.now();
    const activeId = this.getActiveTerminalId();

    // Bug #272: Re-arm threshold — how long a terminal must be truly silent
    // (no output at all) before it can be re-notified. Uses max of
    // 2×cooldown and idleThreshold to ensure periodic shell noise doesn't
    // trigger repeated notifications after each cooldown expiry.
    const rearmThreshold = Math.max(this.notifyCooldownMs * 2, this.idleThresholdMs);

    for (const [terminalId, tracker] of this.trackers) {
      // Skip the currently focused terminal
      if (terminalId === activeId) continue;

      const idleMs = now - tracker.lastOutputTime;

      const hasEnoughOutput = tracker.outputEventCount >= this.minOutputEvents;

      // Bug #272: Re-arm after extended true silence. Once notified, a terminal
      // must go completely silent for rearmThreshold before it can notify again.
      // This prevents periodic noise from restarting the cycle.
      if (tracker.notified && tracker.outputEventCount === 0) {
        if (idleMs >= rearmThreshold) {
          tracker.notified = false;
        }
        continue;
      }

      // Output arrived while in notified state and terminal is now idle.
      // Check if cooldown has expired to decide whether to re-notify.
      if (tracker.outputEventCount > 0 && tracker.notified && idleMs >= this.idleThresholdMs) {
        if (this.notifyCooldownMs > 0 && tracker.lastNotifiedTime > 0 &&
            (now - tracker.lastNotifiedTime) < this.notifyCooldownMs) {
          // Still in cooldown — preserve outputEventCount so we re-check next tick
          continue;
        }
        // Cooldown expired (or no cooldown configured) — allow re-notification
        tracker.notified = false;
        // Fall through to notification logic below
      }

      // Skip if no recent output, not enough output, or already notified
      if (tracker.outputEventCount === 0 || !hasEnoughOutput || tracker.notified) continue;

      if (idleMs >= this.idleThresholdMs) {
        // Check per-terminal cooldown: suppress repeated notifications
        // from rapid idle cycling (e.g., background cursor activity)
        if (this.notifyCooldownMs > 0 && tracker.lastNotifiedTime > 0 &&
            (now - tracker.lastNotifiedTime) < this.notifyCooldownMs) {
          tracker.outputEventCount = 0;
          tracker.notified = true;
          continue;
        }

        tracker.outputEventCount = 0;
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
