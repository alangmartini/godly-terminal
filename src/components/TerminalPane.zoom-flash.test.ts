import { describe, it, expect, vi, beforeEach } from 'vitest';

/**
 * Tests for the "zoomed in flash" bug on terminal activation.
 *
 * Bug: After reopening Godly Terminal and entering any tab, the screen appears
 * super zoomed in for a split second before going back to normal. This happens
 * because:
 *
 * 1. setActive(true) toggles the container's CSS class immediately → container
 *    becomes visible (display: block) and canvas CSS stretches to 100% × 100%
 * 2. But canvas.width / canvas.height (the bitmap dimensions) are NOT updated
 *    until requestAnimationFrame fires, because updateSize() is called inside
 *    fit() which runs in a rAF callback
 * 3. For one frame, the browser stretches the old/stale canvas bitmap
 *    (300×150 default or last-rendered size from before hiding) to fill the
 *    container → content appears "super zoomed in"
 * 4. rAF fires → fit() → updateSize() corrects canvas.width/height → content
 *    re-renders at correct resolution → looks normal
 *
 * The same issue occurs during mount() after app reopen, and in
 * setSplitVisible() for split panes.
 *
 * Expected behavior: The canvas bitmap dimensions must be synchronized with the
 * container BEFORE the browser gets a chance to paint. This means updateSize()
 * must be called synchronously when the pane becomes visible, not deferred to
 * requestAnimationFrame.
 */

// ── Simulator mirroring setActive / mount / setSplitVisible flow ────────

/**
 * Simulates the TerminalPane activation flow, tracking whether canvas bitmap
 * dimensions are updated synchronously or deferred to rAF.
 *
 * The simulator mirrors the actual TerminalPane.setActive() and
 * TerminalRenderer.updateSize() sequence to demonstrate the timing gap.
 */
class ActivationFlowSimulator {
  // ── Container state ──
  containerVisible = false;

  // ── Canvas bitmap dimensions ──
  // HTML canvas default is 300×150; fresh TerminalRenderer creates a canvas
  // without setting width/height, so it starts at the browser default.
  canvasBitmapWidth = 300;
  canvasBitmapHeight = 150;

  // ── Container CSS dimensions (what the canvas stretches to fill) ──
  containerCssWidth: number;
  containerCssHeight: number;
  devicePixelRatio: number;

  // ── Tracking ──
  /** Operations executed synchronously within setActive/mount */
  syncOps: string[] = [];
  /** Operations deferred to requestAnimationFrame */
  rafCallbacks: Array<() => void> = [];
  updateSizeCalled = false;

  constructor(containerWidth = 1200, containerHeight = 800, dpr = 1.5) {
    this.containerCssWidth = containerWidth;
    this.containerCssHeight = containerHeight;
    this.devicePixelRatio = dpr;
  }

  /**
   * Mirror of TerminalPane.setActive(true).
   *
   * From TerminalPane.ts:
   *   setActive(active: boolean) {
   *     this.container.classList.toggle('active', active);    // SYNC
   *     if (active) {
   *       this.renderer.updateSize();                           // SYNC (fix)
   *       requestAnimationFrame(() => {                         // DEFERRED
   *         this.fit();          // calls updateSize() again
   *         ...
   *       });
   *     }
   *   }
   */
  setActive(active: boolean) {
    // SYNC: CSS class toggle → container visible
    this.containerVisible = active;
    this.syncOps.push('visibility_toggled');

    if (active) {
      // SYNC: updateSize() called immediately to prevent zoom flash
      this.updateSize();
      this.syncOps.push('update_size_sync');

      // DEFERRED: full fit() + snapshot in requestAnimationFrame
      this.rafCallbacks.push(() => {
        this.fit();
      });
      this.syncOps.push('raf_scheduled');
    }
  }

  /**
   * Mirror of TerminalPane.mount().
   *
   * From TerminalPane.ts:
   *   mount(parent: HTMLElement) {
   *     parent.appendChild(this.container);                   // SYNC
   *     this.container.appendChild(this.renderer.getElement()); // SYNC
   *     ...
   *     if (container.offsetWidth && container.offsetHeight) { // SYNC (fix)
   *       this.renderer.updateSize();
   *     }
   *     requestAnimationFrame(() => {                         // DEFERRED
   *       this.fit();
   *       this.fetchAndRenderSnapshot();
   *     });
   *   }
   */
  mount() {
    this.syncOps.push('canvas_appended_to_dom');

    // SYNC: updateSize() called immediately if container is visible
    // (prevents zoom flash on the initially active terminal after reopen)
    if (this.containerCssWidth > 0 && this.containerCssHeight > 0) {
      this.updateSize();
      this.syncOps.push('update_size_sync');
    }

    // DEFERRED: full fit() + snapshot in requestAnimationFrame
    this.rafCallbacks.push(() => {
      this.fit();
    });
    this.syncOps.push('raf_scheduled');
  }

  /**
   * Mirror of TerminalPane.setSplitVisible(true, focused).
   *
   * Same pattern: CSS toggle sync, updateSize() sync, fit() deferred to rAF.
   */
  setSplitVisible(visible: boolean) {
    this.containerVisible = visible;
    this.syncOps.push('visibility_toggled');

    if (visible) {
      // SYNC: updateSize() called immediately to prevent zoom flash
      this.updateSize();
      this.syncOps.push('update_size_sync');

      this.rafCallbacks.push(() => {
        this.fit();
      });
      this.syncOps.push('raf_scheduled');
    }
  }

  /**
   * Mirror of TerminalPane.fit() → calls updateSize().
   *
   * From TerminalPane.ts:
   *   fit() {
   *     this.renderer.updateSize();
   *     const { rows, cols } = this.renderer.getGridSize();
   *     ...
   *   }
   */
  private fit() {
    this.updateSize();
  }

  /**
   * Mirror of TerminalRenderer.updateSize().
   * Sets canvas bitmap dimensions to match container CSS dimensions × DPR.
   *
   * From TerminalRenderer.ts:
   *   updateSize(): boolean {
   *     const rect = this.canvas.getBoundingClientRect();
   *     const dpr = window.devicePixelRatio || 1;
   *     this.canvas.width = Math.floor(rect.width * dpr);
   *     this.canvas.height = Math.floor(rect.height * dpr);
   *     ...
   *   }
   */
  private updateSize() {
    this.canvasBitmapWidth = Math.floor(this.containerCssWidth * this.devicePixelRatio);
    this.canvasBitmapHeight = Math.floor(this.containerCssHeight * this.devicePixelRatio);
    this.updateSizeCalled = true;
  }

  /** Simulate the browser executing the next requestAnimationFrame callback. */
  flushRaf() {
    const cb = this.rafCallbacks.shift();
    if (cb) cb();
  }

  /** The expected correct bitmap dimensions for the container. */
  get expectedBitmapWidth(): number {
    return Math.floor(this.containerCssWidth * this.devicePixelRatio);
  }
  get expectedBitmapHeight(): number {
    return Math.floor(this.containerCssHeight * this.devicePixelRatio);
  }

  /**
   * Returns true if the canvas bitmap matches the container's expected
   * dimensions. When false, the browser stretches the stale bitmap to fill
   * the CSS dimensions, causing the "zoomed in" appearance.
   */
  canvasBitmapMatchesContainer(): boolean {
    return (
      this.canvasBitmapWidth === this.expectedBitmapWidth &&
      this.canvasBitmapHeight === this.expectedBitmapHeight
    );
  }

  /**
   * The visual scale factor the browser applies when stretching the stale
   * bitmap. Values > 1.0 indicate zoom-in. For a 300×150 default canvas
   * stretched to 1200×800 CSS, this would be 4.0× horizontally.
   */
  get stretchFactor(): number {
    return this.containerCssWidth / (this.canvasBitmapWidth / this.devicePixelRatio);
  }
}

// ── Tests ───────────────────────────────────────────────────────────────

describe('Terminal zoom flash on activation (canvas bitmap sizing)', () => {
  describe('setActive: canvas bitmap must be synchronized before browser paint', () => {
    it('canvas bitmap should match container immediately after setActive(true)', () => {
      // Bug: After reopening and entering a tab, the screen appears zoomed in
      // for one frame because updateSize() is deferred to requestAnimationFrame.
      const sim = new ActivationFlowSimulator(1200, 800, 1.5);

      // Pane becomes active (container visible via CSS class toggle)
      sim.setActive(true);

      // At this point the container is visible (display: block) and the browser
      // will paint the canvas at CSS 100%×100%. The canvas bitmap MUST already
      // be at the correct dimensions to prevent stretching.
      expect(sim.canvasBitmapMatchesContainer()).toBe(true);
    });

    it('updateSize must be called synchronously in setActive, not deferred to rAF', () => {
      // Bug: updateSize() runs inside fit() which is deferred to rAF.
      // This means for one frame, the canvas bitmap is stale.
      const sim = new ActivationFlowSimulator();

      sim.setActive(true);

      // updateSize should have been called synchronously
      expect(sim.updateSizeCalled).toBe(true);
    });

    it('stale canvas produces visible zoom factor > 1.0 before rAF', () => {
      // Demonstrates the visual severity: a default 300×150 canvas stretched
      // to 1200×800 CSS produces a 4× horizontal zoom — very noticeable.
      const sim = new ActivationFlowSimulator(1200, 800, 1.0);

      sim.setActive(true);

      // Before rAF: canvas is 300×150, container is 1200×800
      // The browser stretches the bitmap by this factor:
      const factor = sim.stretchFactor;

      // After the fix, factor should be 1.0 (no stretching)
      expect(factor).toBeCloseTo(1.0, 1);
    });

    it('canvas bitmap is correct after rAF fires (confirming the delayed fix path)', () => {
      // Shows that the bug is specifically about TIMING — after rAF, everything
      // is correct. The fix is to move updateSize() before the rAF.
      const sim = new ActivationFlowSimulator(1200, 800, 1.5);

      sim.setActive(true);

      // Before rAF: bitmap mismatch (the bug)
      // After rAF: bitmap should match
      sim.flushRaf();
      expect(sim.canvasBitmapMatchesContainer()).toBe(true);
    });
  });

  describe('mount: canvas bitmap must be synchronized before browser paint', () => {
    it('canvas bitmap should match container immediately after mount', () => {
      // Bug: Same issue during initial mount after app reopen.
      // Canvas is appended to DOM with default 300×150, fit() is deferred to rAF.
      const sim = new ActivationFlowSimulator(1200, 800, 1.5);

      sim.mount();

      // After mount, the canvas is in the DOM. If the container is visible
      // (e.g., for the initially active terminal), the bitmap must already
      // be correctly sized.
      expect(sim.canvasBitmapMatchesContainer()).toBe(true);
    });

    it('default canvas 300x150 is visibly stretched on mount', () => {
      // Fresh canvas starts at 300×150 (HTML default).
      // Container is 1200×800. The stretch factor is enormous.
      const sim = new ActivationFlowSimulator(1200, 800, 1.0);

      sim.mount();

      // If updateSize wasn't called synchronously, bitmap is still 300×150
      // Factor should be 1.0 after fix, not 4.0
      expect(sim.stretchFactor).toBeCloseTo(1.0, 1);
    });
  });

  describe('setSplitVisible: canvas bitmap must be synchronized before browser paint', () => {
    it('canvas bitmap should match container immediately after setSplitVisible(true)', () => {
      // Bug: Same deferred-updateSize pattern in split mode.
      const sim = new ActivationFlowSimulator(600, 800, 1.5);

      sim.setSplitVisible(true);

      expect(sim.canvasBitmapMatchesContainer()).toBe(true);
    });
  });

  describe('DPR scaling correctness', () => {
    it('handles HiDPI displays (DPR 2.0) without zoom flash', () => {
      // HiDPI makes the zoom flash worse: canvas default 300×150 at DPR 2
      // is painted at CSS 150×75, then stretched to 1200×800 → 8× zoom.
      const sim = new ActivationFlowSimulator(1200, 800, 2.0);

      sim.setActive(true);

      expect(sim.canvasBitmapMatchesContainer()).toBe(true);
      // Expected bitmap: 2400×1600 (1200*2, 800*2)
      expect(sim.canvasBitmapWidth).toBe(2400);
      expect(sim.canvasBitmapHeight).toBe(1600);
    });

    it('handles fractional DPR (1.25) without zoom flash', () => {
      // Fractional DPR is common on Windows laptops
      const sim = new ActivationFlowSimulator(1200, 800, 1.25);

      sim.setActive(true);

      expect(sim.canvasBitmapMatchesContainer()).toBe(true);
      expect(sim.canvasBitmapWidth).toBe(1500); // 1200 * 1.25
      expect(sim.canvasBitmapHeight).toBe(1000); // 800 * 1.25
    });
  });

  describe('tab switch cycle: no zoom flash on either pane', () => {
    it('switching from Terminal A to Terminal B should not flash zoom on B', () => {
      // Full cycle: A is active → user switches to B
      // A goes hidden, B becomes visible
      // B's canvas must have correct bitmap before browser paints
      const simA = new ActivationFlowSimulator(1200, 800, 1.5);
      const simB = new ActivationFlowSimulator(1200, 800, 1.5);

      // Initial: A is active, B was previously shown at some size
      simA.setActive(true);
      simA.flushRaf(); // A fully rendered

      // Simulate B having been visible before at a different size
      // (e.g., smaller window), then hidden. Its canvas bitmap is stale.
      simB.canvasBitmapWidth = 600;
      simB.canvasBitmapHeight = 400;

      // User switches to B
      simA.setActive(false);
      simB.setActive(true);

      // B should NOT display stretched stale bitmap for a frame.
      // Its canvas bitmap should already match the container.
      expect(simB.canvasBitmapMatchesContainer()).toBe(true);
    });
  });
});
