/**
 * Tracks Claude Code message boundaries in terminal grid output and records
 * timestamps for when each message boundary was first seen.
 *
 * Claude Code uses box-drawing characters to delimit messages:
 *   ╭─  (U+256D U+2500)  — message start / header line
 *
 * This tracker scans each rendered grid snapshot for these patterns and
 * records the wallclock time when each boundary first appears. The overlay
 * renderer uses these timestamps to draw subtle annotations.
 */

import type { RichGridData } from './TerminalRenderer';

export interface MessageBoundary {
  /** Row index in the current viewport. */
  row: number;
  /** Wallclock time when this boundary was first observed. */
  timestamp: number;
}

/**
 * Detect Claude Code message boundary rows in a grid snapshot.
 *
 * A boundary row starts with '╭' followed by one or more '─' characters.
 * We scan the first few columns to detect this pattern.
 */
function detectBoundaryRows(snapshot: RichGridData): number[] {
  const rows: number[] = [];
  for (let r = 0; r < snapshot.rows.length; r++) {
    const cells = snapshot.rows[r].cells;
    if (cells.length < 2) continue;

    const c0 = cells[0]?.content;
    const c1 = cells[1]?.content;

    // Pattern: ╭─  (message start)
    if (c0 === '╭' && c1 === '─') {
      rows.push(r);
    }
  }
  return rows;
}

/**
 * Per-terminal tracker that maintains a stable mapping between message
 * boundary content positions (absolute row index in scrollback) and their
 * first-observed timestamps.
 *
 * Boundaries are keyed by their absolute row position (viewport row +
 * scrollback offset) so that scrolling doesn't create duplicate entries.
 */
export class MessageTimestampTracker {
  /**
   * Map from absolute row index (viewport row + scrollback_offset) to the
   * wallclock time when the boundary was first seen.
   */
  private timestamps = new Map<number, number>();

  /** Maximum number of tracked boundaries before pruning old entries. */
  private static readonly MAX_ENTRIES = 500;

  /**
   * Update the tracker with a new grid snapshot.
   * Returns the list of message boundaries visible in the current viewport.
   */
  update(snapshot: RichGridData): MessageBoundary[] {
    const boundaryRows = detectBoundaryRows(snapshot);
    const now = Date.now();
    const offset = snapshot.scrollback_offset;

    for (const row of boundaryRows) {
      const absRow = row + offset;
      if (!this.timestamps.has(absRow)) {
        this.timestamps.set(absRow, now);
      }
    }

    // Prune if too many entries (keep most recent)
    if (this.timestamps.size > MessageTimestampTracker.MAX_ENTRIES) {
      const entries = [...this.timestamps.entries()]
        .sort((a, b) => a[1] - b[1]);
      const toRemove = entries.slice(0, entries.length - MessageTimestampTracker.MAX_ENTRIES);
      for (const [key] of toRemove) {
        this.timestamps.delete(key);
      }
    }

    // Return boundaries visible in the current viewport
    return boundaryRows.map(row => ({
      row,
      timestamp: this.timestamps.get(row + offset) ?? now,
    }));
  }

  /** Clear all tracked timestamps (e.g. on terminal reset). */
  clear(): void {
    this.timestamps.clear();
  }
}

/**
 * Format a timestamp for display.
 * - Under 60s: "just now"
 * - Under 1h: "Xm ago"
 * - Same day: "HH:MM"
 * - Older: "Mon HH:MM"
 */
export function formatTimestamp(timestamp: number, now?: number): string {
  const current = now ?? Date.now();
  const diffMs = current - timestamp;
  const diffSec = Math.floor(diffMs / 1000);
  const diffMin = Math.floor(diffSec / 60);

  if (diffSec < 60) return 'just now';
  if (diffMin < 60) return `${diffMin}m ago`;

  const date = new Date(timestamp);
  const hours = date.getHours().toString().padStart(2, '0');
  const minutes = date.getMinutes().toString().padStart(2, '0');

  const today = new Date(current);
  if (date.toDateString() === today.toDateString()) {
    return `${hours}:${minutes}`;
  }

  const days = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];
  return `${days[date.getDay()]} ${hours}:${minutes}`;
}
