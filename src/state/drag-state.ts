/**
 * Shared pointer-event drag state for cross-component DnD.
 *
 * HTML5 DnD (draggable / ondragstart / ondrop) is broken when Tauri's
 * dragDropEnabled is true (IDropTarget intercepts the OLE pipeline).
 * This module replaces it with a pointer-event-based approach.
 */

export type DragKind = 'tab' | 'workspace';

export interface DragData {
  kind: DragKind;
  id: string;
  sourceElement: HTMLElement;
}

type MoveHandler = (x: number, y: number, data: DragData) => void;
type DropHandler = (x: number, y: number, data: DragData) => void;

// ── Singleton drag state ──────────────────────────────────────────

let activeDrag: DragData | null = null;

export function startDrag(data: DragData): void {
  activeDrag = data;
  document.body.classList.add('dragging-active');
}

export function getDrag(): DragData | null {
  return activeDrag;
}

export function endDrag(): void {
  activeDrag = null;
  removeGhost();
  document.body.classList.remove('dragging-active');
}

// ── Ghost element ─────────────────────────────────────────────────

let ghost: HTMLElement | null = null;

export function createGhost(sourceEl: HTMLElement): void {
  removeGhost();
  ghost = sourceEl.cloneNode(true) as HTMLElement;
  ghost.className = 'drag-ghost';
  // Match source dimensions
  const rect = sourceEl.getBoundingClientRect();
  ghost.style.width = `${rect.width}px`;
  ghost.style.height = `${rect.height}px`;
  document.body.appendChild(ghost);
}

export function moveGhost(x: number, y: number): void {
  if (!ghost) return;
  ghost.style.left = `${x - ghost.offsetWidth / 2}px`;
  ghost.style.top = `${y - ghost.offsetHeight / 2}px`;
}

export function removeGhost(): void {
  if (ghost) {
    ghost.remove();
    ghost = null;
  }
}

// ── Cross-component callback registry ─────────────────────────────

const moveHandlers: Set<MoveHandler> = new Set();
const dropHandlers: Set<DropHandler> = new Set();

export function onDragMove(handler: MoveHandler): () => void {
  moveHandlers.add(handler);
  return () => { moveHandlers.delete(handler); };
}

export function onDragDrop(handler: DropHandler): () => void {
  dropHandlers.add(handler);
  return () => { dropHandlers.delete(handler); };
}

export function notifyMove(x: number, y: number): void {
  if (!activeDrag) return;
  for (const h of moveHandlers) h(x, y, activeDrag);
}

export function notifyDrop(x: number, y: number): void {
  if (!activeDrag) return;
  for (const h of dropHandlers) h(x, y, activeDrag);
}
