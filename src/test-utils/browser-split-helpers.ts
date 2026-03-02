/**
 * Shared browser test helpers for SplitContainer tests.
 *
 * Provides tree builders, mock pane factories, and assertion helpers
 * for tests that run in real Chromium via Vitest Browser Mode.
 */
import { vi } from 'vitest';
import { SplitContainer, SplitPaneHandle } from '../components/SplitContainer';
import { LayoutNode, LeafNode, SplitNode, GridNode } from '../state/split-types';

// ---------------------------------------------------------------------------
// Tree builders
// ---------------------------------------------------------------------------

export function leaf(id: string): LeafNode {
  return { type: 'leaf', terminal_id: id };
}

export function split(
  dir: 'horizontal' | 'vertical',
  first: LayoutNode,
  second: LayoutNode,
  ratio = 0.5,
): SplitNode {
  return { type: 'split', direction: dir, ratio, first, second };
}

export function grid(
  tl: LayoutNode,
  tr: LayoutNode,
  bl: LayoutNode,
  br: LayoutNode,
  colRatios: [number, number] = [0.5, 0.5],
  rowRatios: [number, number] = [0.5, 0.5],
): GridNode {
  return { type: 'grid', colRatios, rowRatios, children: [tl, tr, bl, br] };
}

// ---------------------------------------------------------------------------
// Mock pane factory
// ---------------------------------------------------------------------------

export function createMockPane(id: string): SplitPaneHandle {
  const container = document.createElement('div');
  container.className = 'terminal-pane';
  container.dataset.id = id;
  container.style.minWidth = '50px';
  container.style.minHeight = '50px';
  return {
    getContainer: () => container,
    setSplitVisible: vi.fn(),
    setActive: vi.fn(),
  };
}

export function createMockPaneMap(ids: string[]): Map<string, SplitPaneHandle> {
  const map = new Map<string, SplitPaneHandle>();
  for (const id of ids) {
    map.set(id, createMockPane(id));
  }
  return map;
}

// ---------------------------------------------------------------------------
// Mount helper
// ---------------------------------------------------------------------------

/** CSS that matches the production split layout from SplitContainer.ts. */
const SPLIT_CSS = `
  .split-root { width: 800px; height: 600px; position: relative; }
  .split-container { display: flex; width: 100%; height: 100%; }
  .split-container.horizontal { flex-direction: row; }
  .split-container.vertical { flex-direction: column; }
  .split-divider { flex-shrink: 0; }
  .split-divider.horizontal { width: 2px; cursor: col-resize; }
  .split-divider.vertical { height: 2px; cursor: row-resize; }
  .terminal-pane { overflow: hidden; }
  .split-grid { position: relative; width: 100%; height: 100%; overflow: hidden; }
  .grid-cell { position: absolute; overflow: hidden; }
  .grid-cell > * { width: 100%; height: 100%; }
  .split-grid-divider { position: absolute; z-index: 1; }
  .split-grid-divider.horizontal { cursor: col-resize; }
  .split-grid-divider.vertical { cursor: row-resize; }
`;

/**
 * Mount a SplitContainer into the real DOM with production-matching CSS.
 * Returns the root element and a cleanup function.
 */
export function mountSplitContainer(sc: SplitContainer): { root: HTMLElement; cleanup: () => void } {
  const root = sc.getElement();

  const style = document.createElement('style');
  style.textContent = SPLIT_CSS;
  document.head.appendChild(style);
  document.body.appendChild(root);

  return {
    root,
    cleanup: () => {
      sc.destroy();
      style.remove();
    },
  };
}

// ---------------------------------------------------------------------------
// Focus assertion helpers
// ---------------------------------------------------------------------------

/**
 * Assert that the given pane is the only focused pane.
 * Checks that `setSplitVisible(true, true)` was called for `expectedId`
 * and `setSplitVisible(true, false)` for all others in the map.
 */
export function assertFocusedPane(
  paneMap: Map<string, SplitPaneHandle>,
  expectedId: string,
): void {
  for (const [id, pane] of paneMap) {
    const mock = vi.mocked(pane.setSplitVisible);
    const lastCall = mock.mock.calls[mock.mock.calls.length - 1];
    if (id === expectedId) {
      if (!lastCall || lastCall[0] !== true || lastCall[1] !== true) {
        throw new Error(
          `Expected pane "${id}" to be focused (setSplitVisible(true, true)), ` +
          `but last call was ${lastCall ? `(${lastCall.join(', ')})` : 'never called'}`,
        );
      }
    } else {
      if (!lastCall || lastCall[0] !== true || lastCall[1] !== false) {
        throw new Error(
          `Expected pane "${id}" to be unfocused (setSplitVisible(true, false)), ` +
          `but last call was ${lastCall ? `(${lastCall.join(', ')})` : 'never called'}`,
        );
      }
    }
  }
}

/** Wait for DOM layout to settle. */
export function waitForLayout(): Promise<void> {
  return new Promise(r => requestAnimationFrame(() => r()));
}
