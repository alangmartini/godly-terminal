import { describe, it, expect, beforeEach } from 'vitest';
import { MessageTimestampTracker, formatTimestamp } from './MessageTimestampTracker';
import type { RichGridData, RichGridRow, RichGridCell } from './TerminalRenderer';

function makeCell(content: string): RichGridCell {
  return {
    content,
    fg: 'default',
    bg: 'default',
    bold: false,
    dim: false,
    italic: false,
    underline: false,
    inverse: false,
    wide: false,
    wide_continuation: false,
  };
}

function makeRow(text: string): RichGridRow {
  return {
    cells: [...text].map(ch => makeCell(ch)),
    wrapped: false,
  };
}

function makeSnapshot(rows: string[], scrollback_offset = 0): RichGridData {
  return {
    rows: rows.map(r => makeRow(r)),
    cursor: { row: 0, col: 0 },
    dimensions: { rows: rows.length, cols: rows[0]?.length ?? 80 },
    alternate_screen: false,
    cursor_hidden: false,
    title: '',
    scrollback_offset,
    total_scrollback: 0,
  };
}

describe('MessageTimestampTracker', () => {
  let tracker: MessageTimestampTracker;

  beforeEach(() => {
    tracker = new MessageTimestampTracker();
  });

  it('detects ╭─ boundary rows', () => {
    const snap = makeSnapshot([
      '╭─ Message',
      '│ Hello',
      '╰─────────',
      '╭─ Another',
    ]);
    const boundaries = tracker.update(snap);
    expect(boundaries).toHaveLength(2);
    expect(boundaries[0].row).toBe(0);
    expect(boundaries[1].row).toBe(3);
  });

  it('ignores rows without boundary pattern', () => {
    const snap = makeSnapshot([
      '│ Some content',
      '╰─────────',
      'regular text',
      '  ╭─ indented',
    ]);
    const boundaries = tracker.update(snap);
    // ╰─ is not a start boundary, indented ╭ doesn't start at col 0
    expect(boundaries).toHaveLength(0);
  });

  it('preserves timestamps across updates', () => {
    const snap1 = makeSnapshot(['╭─ First']);
    const b1 = tracker.update(snap1);
    const ts1 = b1[0].timestamp;

    // Same content, same absolute position — timestamp should be preserved
    const snap2 = makeSnapshot(['╭─ First']);
    const b2 = tracker.update(snap2);
    expect(b2[0].timestamp).toBe(ts1);
  });

  it('tracks boundaries by absolute position (viewport + scrollback)', () => {
    // Boundary at viewport row 0, scrollback offset 10 → absolute row 10
    const snap1 = makeSnapshot(['╭─ First', 'text'], 10);
    const b1 = tracker.update(snap1);
    expect(b1).toHaveLength(1);
    const ts1 = b1[0].timestamp;

    // Same boundary scrolled: now at viewport row 1, scrollback offset 9 → absolute row 10
    const snap2 = makeSnapshot(['old line', '╭─ First'], 9);
    const b2 = tracker.update(snap2);
    expect(b2).toHaveLength(1);
    expect(b2[0].row).toBe(1);
    expect(b2[0].timestamp).toBe(ts1);
  });

  it('clear() removes all tracked timestamps', () => {
    const snap = makeSnapshot(['╭─ Message']);
    tracker.update(snap);
    tracker.clear();

    // After clear, same boundary gets a new timestamp
    const b = tracker.update(snap);
    expect(b).toHaveLength(1);
    // Timestamp is fresh (close to now)
    expect(Date.now() - b[0].timestamp).toBeLessThan(100);
  });

  it('handles empty snapshots', () => {
    const snap = makeSnapshot([]);
    const boundaries = tracker.update(snap);
    expect(boundaries).toHaveLength(0);
  });

  it('handles rows with fewer than 2 cells', () => {
    const snap: RichGridData = {
      rows: [{ cells: [makeCell('╭')], wrapped: false }],
      cursor: { row: 0, col: 0 },
      dimensions: { rows: 1, cols: 1 },
      alternate_screen: false,
      cursor_hidden: false,
      title: '',
      scrollback_offset: 0,
      total_scrollback: 0,
    };
    const boundaries = tracker.update(snap);
    expect(boundaries).toHaveLength(0);
  });
});

describe('formatTimestamp', () => {
  it('shows "just now" for timestamps under 60s', () => {
    const now = Date.now();
    expect(formatTimestamp(now, now)).toBe('just now');
    expect(formatTimestamp(now - 30_000, now)).toBe('just now');
    expect(formatTimestamp(now - 59_000, now)).toBe('just now');
  });

  it('shows "Xm ago" for timestamps under 1h', () => {
    const now = Date.now();
    expect(formatTimestamp(now - 60_000, now)).toBe('1m ago');
    expect(formatTimestamp(now - 5 * 60_000, now)).toBe('5m ago');
    expect(formatTimestamp(now - 59 * 60_000, now)).toBe('59m ago');
  });

  it('shows HH:MM for same-day timestamps over 1h old', () => {
    // Create a timestamp 2h ago, same day
    const now = new Date();
    now.setHours(14, 30, 0, 0);
    const twoHoursAgo = new Date(now);
    twoHoursAgo.setHours(12, 15, 0, 0);

    // Only test if both are on the same day
    if (now.toDateString() === twoHoursAgo.toDateString()) {
      const result = formatTimestamp(twoHoursAgo.getTime(), now.getTime());
      expect(result).toBe('12:15');
    }
  });

  it('shows "Day HH:MM" for timestamps on a different day', () => {
    const now = new Date('2026-03-02T10:00:00');
    const yesterday = new Date('2026-03-01T15:30:00');
    const result = formatTimestamp(yesterday.getTime(), now.getTime());
    // March 1, 2026 is a Sunday
    expect(result).toBe('Sun 15:30');
  });
});
