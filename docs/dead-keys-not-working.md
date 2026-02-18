# Dead Keys Not Working (Quotes on ABNT2 Keyboard)

**Status**: Resolved
**Branch**: `wt-aspas`
**Regression Risk**: Medium — any change to keyboard input handling in TerminalPane could regress this.

## Symptom

On Brazilian/Portuguese (ABNT2) keyboards, quote characters (`'` and `"`) cannot be typed. These keys are dead keys that produce accented characters when combined with vowels (e.g., `' + a = á`).

## Root Cause

The `<canvas>` element doesn't participate in OS text composition. When a dead key is pressed:

1. Browser fires `keydown` with `event.key = "Dead"` (length 4)
2. `keyToTerminalData()` checks `event.key.length === 1` for printable chars — fails
3. Returns `null` — the character is never sent to the PTY
4. No `input` event fires on the canvas because canvas is not an editable element

The composed character (e.g., `'` from dead key + Space) is lost entirely.

## Fix

Replaced canvas-based keyboard input with a hidden `<textarea>`:

1. **Hidden textarea** (`TerminalPane.inputTextarea`): Positioned off-screen, receives all keyboard events. The OS input method properly resolves dead keys and IME sequences on textarea elements.

2. **Input event pipeline**: Printable characters (including dead-key-composed text) flow through the textarea's `input` event instead of `keydown`. The `keyToTerminalData()` method no longer handles printable characters.

3. **Special keys stay in keydown**: Enter, Backspace, arrows, Ctrl/Alt combos are still handled in `keydown` with `preventDefault()`, which prevents them from reaching the textarea's `input` event.

4. **IME composition tracking**: `compositionstart`/`compositionend` events prevent intermediate IME text from being sent to the terminal.

### Files Changed

- `src/components/TerminalPane.ts` — Hidden textarea creation, input/composition event handlers, focus management redirected from canvas to textarea
- `src/components/TerminalPane.dead-keys.test.ts` — 25 tests covering dead keys, IME, and the textarea input pipeline

## Verification

- TypeScript: `tsc --noEmit` passes
- Tests: All 365 tests pass (26 files)
- Manual: Test with ABNT2 layout — `'`, `"`, and accented characters (`á`, `é`, `ã`) should work

## Attempts

| # | Approach | Result |
|---|----------|--------|
| 1 | Hidden textarea for input capture | Resolved — dead keys compose correctly via textarea's input event |
