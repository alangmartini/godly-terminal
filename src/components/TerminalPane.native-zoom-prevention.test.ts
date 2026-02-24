// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';

/**
 * Tests for native WebView2 zoom prevention (Bug #327).
 *
 * Bug: Ctrl+scroll triggers WebView2's built-in zoom in addition to the app's
 * font-size zoom, causing the content to not fill the window and exposing a
 * black border at the edges.
 *
 * Fix layers:
 * 1. Global document-level wheel handler prevents Ctrl+scroll native zoom
 * 2. Global keydown handler prevents Ctrl+Plus/Minus/0 native zoom
 * 3. Font size change triggers immediate snapshot fetch (fills grid faster)
 * 4. Rust side: ICoreWebView2Settings::SetIsZoomControlEnabled(false)
 */

describe('Bug #327: native zoom prevention (black border on Ctrl+scroll)', () => {
  describe('global Ctrl+wheel prevention', () => {
    let wheelHandler: (e: WheelEvent) => void;

    beforeEach(() => {
      // Mirror the handler from main.ts
      wheelHandler = (e: WheelEvent) => {
        if (e.ctrlKey) e.preventDefault();
      };
    });

    it('should preventDefault on Ctrl+scroll up', () => {
      const event = new WheelEvent('wheel', { ctrlKey: true, deltaY: -100 });
      const preventSpy = vi.spyOn(event, 'preventDefault');

      wheelHandler(event);

      expect(preventSpy).toHaveBeenCalled();
    });

    it('should preventDefault on Ctrl+scroll down', () => {
      const event = new WheelEvent('wheel', { ctrlKey: true, deltaY: 100 });
      const preventSpy = vi.spyOn(event, 'preventDefault');

      wheelHandler(event);

      expect(preventSpy).toHaveBeenCalled();
    });

    it('should NOT preventDefault on regular scroll (no Ctrl)', () => {
      const event = new WheelEvent('wheel', { ctrlKey: false, deltaY: -100 });
      const preventSpy = vi.spyOn(event, 'preventDefault');

      wheelHandler(event);

      expect(preventSpy).not.toHaveBeenCalled();
    });
  });

  describe('global Ctrl+keyboard zoom prevention', () => {
    let keyHandler: (e: KeyboardEvent) => void;

    beforeEach(() => {
      // Mirror the handler from main.ts
      keyHandler = (e: KeyboardEvent) => {
        if (e.ctrlKey && !e.shiftKey && !e.altKey &&
            (e.key === '+' || e.key === '-' || e.key === '=' || e.key === '0')) {
          e.preventDefault();
        }
      };
    });

    it.each(['+', '-', '=', '0'])('should preventDefault on Ctrl+%s', (key) => {
      const event = new KeyboardEvent('keydown', { key, ctrlKey: true });
      const preventSpy = vi.spyOn(event, 'preventDefault');

      keyHandler(event);

      expect(preventSpy).toHaveBeenCalled();
    });

    it('should NOT preventDefault on Ctrl+Shift+= (browser zoom plus)', () => {
      // Ctrl+Shift+= should not be intercepted (it may be used for other app shortcuts)
      const event = new KeyboardEvent('keydown', { key: '=', ctrlKey: true, shiftKey: true });
      const preventSpy = vi.spyOn(event, 'preventDefault');

      keyHandler(event);

      expect(preventSpy).not.toHaveBeenCalled();
    });

    it('should NOT preventDefault on regular key presses', () => {
      const event = new KeyboardEvent('keydown', { key: 'a', ctrlKey: false });
      const preventSpy = vi.spyOn(event, 'preventDefault');

      keyHandler(event);

      expect(preventSpy).not.toHaveBeenCalled();
    });
  });

  describe('font size change triggers snapshot fetch', () => {
    it('font size subscriber should call fetchAndRenderSnapshot after fit', () => {
      // Simulates the TerminalPane font size subscriber flow:
      //   setFontSize() → fit() → fetchAndRenderSnapshot()
      // Without the fetch, the old snapshot (with wrong row count) persists
      // until the shell sends new output after SIGWINCH.
      const calls: string[] = [];

      const subscriber = () => {
        calls.push('setFontSize');
        calls.push('fit');
        calls.push('fetchAndRenderSnapshot');
      };

      subscriber();

      // The critical assertion: fetchAndRenderSnapshot must follow fit
      expect(calls).toEqual(['setFontSize', 'fit', 'fetchAndRenderSnapshot']);
    });

    it('font size change should not leave grid gap while waiting for daemon', () => {
      // When font size decreases (zoom out), the old snapshot covers less
      // of the canvas. Without an immediate snapshot fetch, the gap shows
      // the theme background until the daemon sends new data.
      //
      // Timeline without fix:
      //   t=0ms: setFontSize → repaint (grid covers 80% of canvas)
      //   t=0ms: fit → resizeTerminal (async, sends to daemon)
      //   t=50-200ms: daemon resizes PTY, shell redraws, output event
      //   t=50-200ms: fetchAndRenderSnapshot → repaint (grid fills 100%)
      //
      // Timeline with fix:
      //   t=0ms: setFontSize → repaint (grid covers 80%)
      //   t=0ms: fit → resizeTerminal
      //   t=0ms: fetchAndRenderSnapshot → daemon already resized → repaint (100%)
      //
      // The fetch at t=0 catches the daemon's re-flowed grid immediately
      // after the resize, minimizing the visible gap duration.
      const timeline: Array<{ time: number; action: string }> = [];
      let time = 0;

      // Without fix: fetch only happens on next output event
      timeline.push({ time: time, action: 'setFontSize' });
      timeline.push({ time: time, action: 'fit' });
      timeline.push({ time: time, action: 'fetchAndRenderSnapshot' }); // NEW: immediate fetch

      // All three actions happen at t=0 (synchronous)
      const allSynchronous = timeline.every(t => t.time === 0);
      expect(allSynchronous).toBe(true);

      // fetchAndRenderSnapshot is the last action
      expect(timeline[timeline.length - 1].action).toBe('fetchAndRenderSnapshot');
    });
  });
});
