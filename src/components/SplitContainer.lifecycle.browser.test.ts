/**
 * SplitContainer lifecycle browser tests — reproduces pane orphaning bug #405.
 *
 * Bug: Split a terminal → create new tab → switch back → nothing shows.
 * Root cause: SplitContainer.destroy() removes the split-root from DOM,
 * which also removes re-parented pane containers. The single-pane rendering
 * code calls setActive(true) on orphaned containers that are no longer in
 * the document tree.
 */
import { describe, it, expect, vi, afterEach } from 'vitest';
import { SplitContainer, SplitPaneHandle } from './SplitContainer';
import {
  leaf,
  split,
  createMockPaneMap,
} from '../test-utils/browser-split-helpers';

/** Minimal CSS for split layout. */
const SPLIT_CSS = `
  .split-root { width: 800px; height: 600px; position: relative; }
  .split-container { display: flex; width: 100%; height: 100%; }
  .split-container.horizontal { flex-direction: row; }
  .split-container.vertical { flex-direction: column; }
  .split-divider { flex-shrink: 0; }
  .split-divider.horizontal { width: 2px; cursor: col-resize; }
  .split-divider.vertical { height: 2px; cursor: row-resize; }
  .terminal-pane { overflow: hidden; min-width: 50px; min-height: 50px; }
`;

/**
 * Simulates the App.ts pane lifecycle:
 * 1. Creates a terminalContainer (like App.ts this.terminalContainer)
 * 2. Mounts pane containers into it (like pane.mount(terminalContainer))
 * 3. Returns helpers to create/destroy splits and check DOM state
 */
function setupAppSimulation(paneIds: string[]) {
  const style = document.createElement('style');
  style.textContent = SPLIT_CSS;
  document.head.appendChild(style);

  // Simulate App.ts terminalContainer
  const terminalContainer = document.createElement('div');
  terminalContainer.className = 'terminal-container';
  terminalContainer.style.width = '800px';
  terminalContainer.style.height = '600px';
  document.body.appendChild(terminalContainer);

  const paneMap = createMockPaneMap(paneIds);

  // Simulate pane.mount(terminalContainer) — App.ts:166
  for (const [, pane] of paneMap) {
    terminalContainer.appendChild(pane.getContainer());
  }

  let splitContainer: SplitContainer | null = null;

  return {
    terminalContainer,
    paneMap,

    /** Simulate App.ts creating a split (lines 195-229). */
    createSplit(layout: ReturnType<typeof split>) {
      splitContainer = new SplitContainer(layout, {
        paneMap,
        onRatioChange: vi.fn(),
        onFocusPane: vi.fn(),
        focusedTerminalId: null,
      });
      terminalContainer.appendChild(splitContainer.getElement());
      return splitContainer;
    },

    /** Simulate App.ts destroySplitContainer() (lines 1023-1028). */
    destroySplit() {
      if (splitContainer) {
        splitContainer.destroy();
        splitContainer = null;
      }
    },

    /**
     * Simulate App.ts single-pane rendering (lines 279-293).
     * Sets the active pane and hides others.
     */
    activateSinglePane(activeId: string) {
      for (const [id, pane] of paneMap) {
        const isActive = id === activeId;
        // Mirrors App.ts: pane.setActive(isVisible)
        pane.setSplitVisible(false, false);
        pane.setActive(isActive);
        pane.getContainer().classList.toggle('active', isActive);
        pane.getContainer().classList.remove('split-visible', 'split-focused');
      }
    },

    cleanup() {
      if (splitContainer) splitContainer.destroy();
      document.body.innerHTML = '';
      style.remove();
    },
  };
}

afterEach(() => {
  document.body.innerHTML = '';
  document.head.querySelectorAll('style').forEach(s => s.remove());
});

// ---------------------------------------------------------------------------
// Bug #405: Pane containers orphaned after split destroy
// ---------------------------------------------------------------------------

describe('Bug #405: split destroy orphans pane containers', () => {
  it('pane containers are in DOM before split is created', () => {
    const app = setupAppSimulation(['t1', 't2']);

    // Before any split, panes should be direct children of terminalContainer
    const t1Container = app.paneMap.get('t1')!.getContainer();
    const t2Container = app.paneMap.get('t2')!.getContainer();

    expect(t1Container.parentElement).toBe(app.terminalContainer);
    expect(t2Container.parentElement).toBe(app.terminalContainer);
    expect(document.body.contains(t1Container)).toBe(true);
    expect(document.body.contains(t2Container)).toBe(true);

    app.cleanup();
  });

  it('pane containers are re-parented into split DOM tree', () => {
    const app = setupAppSimulation(['t1', 't2']);
    const layout = split('horizontal', leaf('t1'), leaf('t2'));

    app.createSplit(layout);

    // After split, panes should be inside the split-root, NOT terminalContainer
    const t1Container = app.paneMap.get('t1')!.getContainer();
    const t2Container = app.paneMap.get('t2')!.getContainer();

    expect(t1Container.parentElement).not.toBe(app.terminalContainer);
    expect(t2Container.parentElement).not.toBe(app.terminalContainer);
    // But still in the document
    expect(document.body.contains(t1Container)).toBe(true);
    expect(document.body.contains(t2Container)).toBe(true);

    app.cleanup();
  });

  it('pane containers remain in DOM after split is destroyed', () => {
    // Bug #405: pane containers orphaned when split is destroyed
    // This is the core reproduction — after destroy, containers should
    // still be in the document so setActive(true) actually works.
    const app = setupAppSimulation(['t1', 't2']);
    const layout = split('horizontal', leaf('t1'), leaf('t2'));

    // Step 1: Create split (re-parents panes into split DOM tree)
    app.createSplit(layout);

    // Step 2: Destroy split (simulates what happens when new terminal is created)
    app.destroySplit();

    // Step 3: Check that pane containers are still in the document
    const t1Container = app.paneMap.get('t1')!.getContainer();
    const t2Container = app.paneMap.get('t2')!.getContainer();

    expect(document.body.contains(t1Container)).toBe(true);
    expect(document.body.contains(t2Container)).toBe(true);

    app.cleanup();
  });

  it('full lifecycle: split → new tab → switch back shows pane', () => {
    // Bug #405: Full user scenario reproduction
    // 1. Split t1 into [t1|t2]
    // 2. Create new terminal t3 (destroys split, activates t3)
    // 3. Switch back to t1 — should be visible
    const app = setupAppSimulation(['t1', 't2', 't3']);
    const layout = split('horizontal', leaf('t1'), leaf('t2'));

    // Step 1: Create split [t1|t2]
    app.createSplit(layout);

    // Step 2: "Create new terminal" — destroys split, shows t3
    app.destroySplit();
    app.activateSinglePane('t3');

    // Verify t3 is visible
    const t3Container = app.paneMap.get('t3')!.getContainer();
    expect(document.body.contains(t3Container)).toBe(true);
    expect(t3Container.classList.contains('active')).toBe(true);

    // Step 3: Switch back to t1
    app.activateSinglePane('t1');

    const t1Container = app.paneMap.get('t1')!.getContainer();
    // t1 should be in the DOM and have the active class
    expect(document.body.contains(t1Container)).toBe(true);
    expect(t1Container.classList.contains('active')).toBe(true);

    // t1 should have real dimensions (visible in layout)
    const rect = t1Container.getBoundingClientRect();
    expect(rect.width).toBeGreaterThan(0);
    expect(rect.height).toBeGreaterThan(0);

    app.cleanup();
  });

  it('full lifecycle: split → new tab → switch back to OTHER split pane', () => {
    // Same as above but switching to t2 (the second pane of the split)
    const app = setupAppSimulation(['t1', 't2', 't3']);
    const layout = split('horizontal', leaf('t1'), leaf('t2'));

    app.createSplit(layout);
    app.destroySplit();
    app.activateSinglePane('t3');

    // Switch to t2
    app.activateSinglePane('t2');

    const t2Container = app.paneMap.get('t2')!.getContainer();
    expect(document.body.contains(t2Container)).toBe(true);
    expect(t2Container.classList.contains('active')).toBe(true);

    const rect = t2Container.getBoundingClientRect();
    expect(rect.width).toBeGreaterThan(0);
    expect(rect.height).toBeGreaterThan(0);

    app.cleanup();
  });

  it('vertical split: pane containers survive destroy', () => {
    const app = setupAppSimulation(['t1', 't2']);
    const layout = split('vertical', leaf('t1'), leaf('t2'));

    app.createSplit(layout);
    app.destroySplit();

    const t1Container = app.paneMap.get('t1')!.getContainer();
    const t2Container = app.paneMap.get('t2')!.getContainer();

    expect(document.body.contains(t1Container)).toBe(true);
    expect(document.body.contains(t2Container)).toBe(true);

    app.cleanup();
  });

  it('nested 3-pane split: all pane containers survive destroy', () => {
    const app = setupAppSimulation(['t1', 't2', 't3']);
    const layout = split('horizontal',
      leaf('t1'),
      split('vertical', leaf('t2'), leaf('t3')),
    );

    app.createSplit(layout);
    app.destroySplit();

    for (const id of ['t1', 't2', 't3']) {
      const container = app.paneMap.get(id)!.getContainer();
      expect(document.body.contains(container)).toBe(true);
    }

    app.cleanup();
  });
});
