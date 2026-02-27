/**
 * SplitContainer — recursively renders a LayoutNode tree into nested
 * flex containers with draggable dividers.
 *
 * Each leaf maps to a TerminalPane (or FigmaPane) from the pane map.
 * Split nodes become flex containers (row for horizontal, column for vertical)
 * with a divider between the two children.
 */
import { LayoutNode, terminalIds } from '../state/split-types';

/** Minimal pane interface — satisfied by both TerminalPane and FigmaPane. */
export interface SplitPaneHandle {
  getContainer(): HTMLElement;
  setSplitVisible(visible: boolean, focused: boolean): void;
  setActive(active: boolean): void;
}

export interface SplitContainerOptions {
  paneMap: Map<string, SplitPaneHandle>;
  onRatioChange: (path: number[], ratio: number) => void;
  onFocusPane: (terminalId: string) => void;
  focusedTerminalId: string | null;
}

/**
 * Rendered node metadata — used for efficient diffing on update.
 */
interface RenderedNode {
  node: LayoutNode;
  element: HTMLElement;
  children?: { first: RenderedNode; second: RenderedNode; divider: HTMLElement };
}

export class SplitContainer {
  private root: RenderedNode | null = null;
  private element: HTMLElement;
  private options: SplitContainerOptions;
  private cleanupFns: (() => void)[] = [];

  constructor(
    private node: LayoutNode,
    options: SplitContainerOptions,
  ) {
    this.options = options;
    this.element = document.createElement('div');
    this.element.className = 'split-root';
    this.root = this.renderNode(node, []);
    this.element.appendChild(this.root.element);
    this.applyPaneVisibility();
  }

  getElement(): HTMLElement {
    return this.element;
  }

  /**
   * Update the tree with a new LayoutNode. Diffs against the current tree
   * and only re-renders changed subtrees.
   */
  update(newNode: LayoutNode, newFocusedId?: string | null): void {
    if (newFocusedId !== undefined) {
      this.options.focusedTerminalId = newFocusedId;
    }

    if (nodesEqual(this.node, newNode)) {
      // Tree structure unchanged — just update focus classes
      this.applyPaneVisibility();
      return;
    }

    this.node = newNode;
    this.cleanup();
    this.root = this.renderNode(newNode, []);
    this.element.innerHTML = '';
    this.element.appendChild(this.root.element);
    this.applyPaneVisibility();
  }

  /** Update only the focused terminal (no structural change). */
  updateFocus(focusedTerminalId: string | null): void {
    this.options.focusedTerminalId = focusedTerminalId;
    this.applyPaneVisibility();
  }

  getNode(): LayoutNode {
    return this.node;
  }

  destroy(): void {
    this.cleanup();
    // Re-parent pane containers back to the split-root's parent before
    // removing the split-root. Without this, pane containers become orphaned
    // and invisible when single-pane mode tries to show them. (Bug #405)
    const parent = this.element.parentElement;
    if (parent) {
      this.restorePaneContainers(this.root, parent);
    }
    this.element.remove();
  }

  // -------------------------------------------------------------------
  // Private
  // -------------------------------------------------------------------

  private renderNode(node: LayoutNode, path: number[]): RenderedNode {
    if (node.type === 'leaf') {
      return this.renderLeaf(node, path);
    }
    return this.renderSplit(node, path);
  }

  private renderLeaf(node: LayoutNode & { type: 'leaf' }, _path: number[]): RenderedNode {
    const pane = this.options.paneMap.get(node.terminal_id);
    if (pane) {
      const container = pane.getContainer();
      container.classList.add('split-pane');
      return { node, element: container };
    }

    // Fallback: create a placeholder if pane doesn't exist yet
    const placeholder = document.createElement('div');
    placeholder.className = 'split-pane split-pane-placeholder';
    placeholder.dataset.terminalId = node.terminal_id;
    return { node, element: placeholder };
  }

  private renderSplit(node: LayoutNode & { type: 'split' }, path: number[]): RenderedNode {
    const container = document.createElement('div');
    container.className = `split-container ${node.direction}`;

    const firstRendered = this.renderNode(node.first, [...path, 0]);
    const divider = this.createDivider(node.direction, node.ratio, path);
    const secondRendered = this.renderNode(node.second, [...path, 1]);

    // Set flex-basis on children
    this.applyFlexBasis(firstRendered.element, node.ratio, node.direction);
    this.applyFlexBasis(secondRendered.element, 1 - node.ratio, node.direction);

    container.appendChild(firstRendered.element);
    container.appendChild(divider);
    container.appendChild(secondRendered.element);

    return {
      node,
      element: container,
      children: { first: firstRendered, second: secondRendered, divider },
    };
  }

  private createDivider(
    direction: 'horizontal' | 'vertical',
    _initialRatio: number,
    path: number[],
  ): HTMLElement {
    const divider = document.createElement('div');
    divider.className = `split-divider ${direction}`;

    const onMouseDown = (e: MouseEvent) => {
      e.preventDefault();
      e.stopPropagation();

      // Walk the tree to find the split node at this path
      const splitNode = this.getNodeAtPath(this.node, path);
      if (!splitNode || splitNode.type !== 'split') return;

      const isHorizontal = splitNode.direction === 'horizontal';

      // Find the container that owns this divider to compute mouse-relative ratio
      const parentContainer = divider.parentElement;
      if (!parentContainer) return;
      const rect = parentContainer.getBoundingClientRect();

      const onMouseMove = (moveEvent: MouseEvent) => {
        let ratio: number;
        if (isHorizontal) {
          ratio = (moveEvent.clientX - rect.left) / rect.width;
        } else {
          ratio = (moveEvent.clientY - rect.top) / rect.height;
        }
        ratio = Math.max(0.15, Math.min(0.85, ratio));

        // Update flex-basis of siblings live for smooth dragging
        const firstChild = divider.previousElementSibling as HTMLElement | null;
        const secondChild = divider.nextElementSibling as HTMLElement | null;
        if (firstChild) this.applyFlexBasis(firstChild, ratio, splitNode.direction);
        if (secondChild) this.applyFlexBasis(secondChild, 1 - ratio, splitNode.direction);

        this.options.onRatioChange(path, ratio);
      };

      const onMouseUp = () => {
        document.removeEventListener('mousemove', onMouseMove);
        document.removeEventListener('mouseup', onMouseUp);
        document.body.classList.remove('split-resizing');
      };

      document.body.classList.add('split-resizing');
      document.addEventListener('mousemove', onMouseMove);
      document.addEventListener('mouseup', onMouseUp);
    };

    divider.addEventListener('mousedown', onMouseDown);
    this.cleanupFns.push(() => divider.removeEventListener('mousedown', onMouseDown));

    return divider;
  }

  /**
   * Walk the tree to find the node at a given path.
   * Path is an array of 0 (first child) or 1 (second child) indices.
   * An empty path returns the root.
   */
  private getNodeAtPath(root: LayoutNode, path: number[]): LayoutNode | null {
    let current: LayoutNode = root;
    for (const idx of path) {
      if (current.type !== 'split') return null;
      current = idx === 0 ? current.first : current.second;
    }
    // The divider is at `path` level — the parent split is one level up
    // But since we store path as the path TO the split, we return the
    // node that _contains_ the split. For a divider created at a split node,
    // the path leads to the split node itself.
    return current;
  }

  private applyFlexBasis(el: HTMLElement, ratio: number, direction: 'horizontal' | 'vertical') {
    // Subtract divider size (2px per divider on this level)
    el.style.flexBasis = `calc(${ratio * 100}% - 1px)`;
    el.style.flexGrow = '0';
    el.style.flexShrink = '0';
    if (direction === 'horizontal') {
      el.style.minWidth = '0';
    } else {
      el.style.minHeight = '0';
    }
  }

  /**
   * Apply split-visible / split-focused classes to all panes based on
   * whether they're in the current tree.
   */
  private applyPaneVisibility() {
    const visibleIds = new Set(terminalIds(this.node));
    const focusedId = this.options.focusedTerminalId;

    for (const [id, pane] of this.options.paneMap) {
      if (visibleIds.has(id)) {
        pane.setSplitVisible(true, id === focusedId);
      }
      // Note: hiding non-visible panes is handled by the caller (App.ts)
      // since the paneMap may contain panes from other workspaces.
    }
  }

  /**
   * Walk the rendered tree and move pane containers (leaf elements that
   * came from the paneMap) back to the given parent element.
   */
  private restorePaneContainers(rendered: RenderedNode | null, parent: HTMLElement): void {
    if (!rendered) return;
    if (rendered.node.type === 'leaf') {
      const pane = this.options.paneMap.get(rendered.node.terminal_id);
      if (pane) {
        parent.appendChild(pane.getContainer());
      }
    } else if (rendered.children) {
      this.restorePaneContainers(rendered.children.first, parent);
      this.restorePaneContainers(rendered.children.second, parent);
    }
  }

  private cleanup() {
    for (const fn of this.cleanupFns) fn();
    this.cleanupFns = [];
  }
}

/** Structural equality check — ignores ratio differences for structure. */
function nodesEqual(a: LayoutNode, b: LayoutNode): boolean {
  if (a.type !== b.type) return false;
  if (a.type === 'leaf' && b.type === 'leaf') {
    return a.terminal_id === b.terminal_id;
  }
  if (a.type === 'split' && b.type === 'split') {
    return (
      a.direction === b.direction &&
      a.ratio === b.ratio &&
      nodesEqual(a.first, b.first) &&
      nodesEqual(a.second, b.second)
    );
  }
  return false;
}
