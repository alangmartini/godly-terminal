# Zoom Flash on Terminal Activation

**Status**: Resolved
**Regression Risk**: Low — change is additive (extra sync updateSize call)

## Symptom

After reopening Godly Terminal and entering any tab, the screen appears "super zoomed in" for a split second before going back to normal. Also visible when switching tabs.

## Root Cause

In `TerminalPane.setActive()`, `setSplitVisible()`, and `mount()`, the container CSS class is toggled immediately (making the canvas visible), but the canvas bitmap size correction (`renderer.updateSize()`) was deferred to `requestAnimationFrame` inside `fit()`.

For one frame, the browser stretches the old/default canvas bitmap (300x150 HTML default or stale from last render) to fill the container via CSS `width:100%; height:100%`, producing a visible zoom flash.

### The timing gap

```
setActive(true)
  ├── classList.toggle('active', true)    ← SYNC: container visible, canvas CSS-stretched
  └── requestAnimationFrame(() => {
        fit()                             ← DEFERRED: canvas.width/height corrected
        fetchAndRenderSnapshot()
      })
```

Between the sync class toggle and the rAF callback, the browser renders one frame with the stretched stale bitmap.

## Fix

Call `renderer.updateSize()` synchronously immediately after the CSS class toggle, before `requestAnimationFrame`. This ensures the canvas bitmap dimensions match the container before the browser paints:

```typescript
setActive(active: boolean) {
    this.container.classList.toggle('active', active);
    if (active) {
      this.renderer.updateSize();  // ← Sync: prevents zoom flash
      requestAnimationFrame(() => {
        this.fit();
        ...
      });
    }
}
```

Applied to: `setActive()`, `setSplitVisible()`, and `mount()` (with visibility guard).

## Files Changed

- `src/components/TerminalPane.ts` — added sync `updateSize()` calls
- `src/components/TerminalPane.zoom-flash.test.ts` — reproduction test suite (10 tests)
