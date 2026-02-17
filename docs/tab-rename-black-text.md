# Tab Rename: Black Text on Dark Background + Enter Double-Fire

**Status**: Resolved
**Branch**: `wt-fix-rename`
**Regression Risk**: Low — CSS-only change + single guard variable

## Symptoms

1. When renaming a tab (double-click or F2), typed text is invisible (black on dark background)
2. Pressing Enter to confirm the rename sometimes fails silently or causes race conditions

## Root Cause

### Black text
The `.tab-title.editing` CSS class set `background: var(--bg-primary)` but never set `color`. Browser `<input>` elements default to black text, which is invisible against `--bg-primary: #1a1b26`.

### Enter double-fire
`finishRename()` was called directly on Enter keydown. It called `this.render()`, which removed the input from the DOM, which triggered `blur`, which called `finishRename()` again. This double-fire caused a race condition where the second call could interfere with the first.

### Input not replaced on render
`updateTabInPlace()` queried `.tab-title` and found the `<input>` element during editing, but never replaced it back with a `<span>`. After both Enter and Escape, the input persisted in the DOM.

## Fix

1. **CSS**: Added `color: var(--text-primary)` plus `font-family: inherit; font-size: inherit` to `.tab-title.editing`
2. **Enter handler**: Changed to call `input.blur()` instead of `finishRename()` directly, so there's only one code path (blur → finishRename)
3. **Guard variable**: Added `finished` boolean to prevent `finishRename()` from running twice
4. **updateTabInPlace**: Added check for `titleEl.tagName === 'INPUT'` to replace stale rename inputs back with spans

## Files Changed

- `src/styles/main.css` — added `color`, `font-family`, `font-size` to `.tab-title.editing`
- `src/components/TabBar.ts` — guard variable, Enter→blur delegation, input→span replacement in render
- `src/components/TabBar.rename.test.ts` — new test suite (6 tests)
