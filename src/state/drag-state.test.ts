// @vitest-environment jsdom
import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
  startDrag, getDrag, endDrag,
  createGhost, moveGhost, removeGhost,
  onDragMove, onDragDrop, notifyMove, notifyDrop,
  DragData,
} from './drag-state';

function makeDrag(overrides: Partial<DragData> = {}): DragData {
  return {
    kind: 'tab',
    id: 't-1',
    sourceElement: document.createElement('div'),
    ...overrides,
  };
}

describe('drag-state singleton', () => {
  beforeEach(() => {
    endDrag(); // clean slate
  });

  it('getDrag returns null when no drag is active', () => {
    expect(getDrag()).toBeNull();
  });

  it('startDrag sets active drag data', () => {
    const data = makeDrag();
    startDrag(data);
    expect(getDrag()).toBe(data);
  });

  it('endDrag clears active drag data', () => {
    startDrag(makeDrag());
    endDrag();
    expect(getDrag()).toBeNull();
  });

  it('startDrag adds dragging-active class to body', () => {
    startDrag(makeDrag());
    expect(document.body.classList.contains('dragging-active')).toBe(true);
  });

  it('endDrag removes dragging-active class from body', () => {
    startDrag(makeDrag());
    endDrag();
    expect(document.body.classList.contains('dragging-active')).toBe(false);
  });
});

describe('ghost element', () => {
  beforeEach(() => {
    endDrag();
    removeGhost();
  });

  it('createGhost appends a .drag-ghost to body', () => {
    const source = document.createElement('div');
    source.textContent = 'Tab';
    source.style.width = '100px';
    source.style.height = '30px';
    document.body.appendChild(source);

    createGhost(source);
    const ghost = document.querySelector('.drag-ghost');
    expect(ghost).toBeTruthy();

    source.remove();
  });

  it('moveGhost updates ghost position', () => {
    const source = document.createElement('div');
    document.body.appendChild(source);
    createGhost(source);

    moveGhost(200, 150);
    const ghost = document.querySelector('.drag-ghost') as HTMLElement;
    expect(ghost).toBeTruthy();
    // Position is set via style.left / style.top
    expect(ghost.style.left).toContain('px');
    expect(ghost.style.top).toContain('px');

    source.remove();
  });

  it('removeGhost removes ghost from DOM', () => {
    const source = document.createElement('div');
    document.body.appendChild(source);
    createGhost(source);
    expect(document.querySelector('.drag-ghost')).toBeTruthy();

    removeGhost();
    expect(document.querySelector('.drag-ghost')).toBeNull();

    source.remove();
  });

  it('endDrag also removes ghost', () => {
    const source = document.createElement('div');
    document.body.appendChild(source);
    startDrag(makeDrag());
    createGhost(source);

    endDrag();
    expect(document.querySelector('.drag-ghost')).toBeNull();

    source.remove();
  });
});

describe('callback registry', () => {
  beforeEach(() => {
    endDrag();
  });

  it('onDragMove handlers are called by notifyMove', () => {
    const handler = vi.fn();
    const unsub = onDragMove(handler);

    const data = makeDrag();
    startDrag(data);
    notifyMove(100, 200);

    expect(handler).toHaveBeenCalledWith(100, 200, data);
    unsub();
  });

  it('onDragDrop handlers are called by notifyDrop', () => {
    const handler = vi.fn();
    const unsub = onDragDrop(handler);

    const data = makeDrag();
    startDrag(data);
    notifyDrop(300, 400);

    expect(handler).toHaveBeenCalledWith(300, 400, data);
    unsub();
  });

  it('unsubscribe prevents further calls', () => {
    const handler = vi.fn();
    const unsub = onDragMove(handler);

    startDrag(makeDrag());
    notifyMove(10, 20);
    expect(handler).toHaveBeenCalledTimes(1);

    unsub();
    notifyMove(30, 40);
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it('notifyMove does nothing when no drag is active', () => {
    const handler = vi.fn();
    const unsub = onDragMove(handler);

    // No startDrag called
    notifyMove(10, 20);
    expect(handler).not.toHaveBeenCalled();

    unsub();
  });

  it('notifyDrop does nothing when no drag is active', () => {
    const handler = vi.fn();
    const unsub = onDragDrop(handler);

    notifyDrop(10, 20);
    expect(handler).not.toHaveBeenCalled();

    unsub();
  });

  it('multiple handlers are all called', () => {
    const h1 = vi.fn();
    const h2 = vi.fn();
    const unsub1 = onDragMove(h1);
    const unsub2 = onDragMove(h2);

    startDrag(makeDrag());
    notifyMove(50, 60);

    expect(h1).toHaveBeenCalledTimes(1);
    expect(h2).toHaveBeenCalledTimes(1);

    unsub1();
    unsub2();
  });

  it('workspace drag data is passed correctly', () => {
    const handler = vi.fn();
    const unsub = onDragDrop(handler);

    const data = makeDrag({ kind: 'workspace', id: 'ws-1' });
    startDrag(data);
    notifyDrop(0, 0);

    expect(handler).toHaveBeenCalledWith(0, 0, expect.objectContaining({
      kind: 'workspace',
      id: 'ws-1',
    }));

    unsub();
  });
});
