# Add Button Overlaps Tabs When Many Tabs Open

## Status: Reproduced (test written, awaiting fix)

## Symptom

When many tabs are open and their combined width exceeds the viewport, the "+" (add tab) button visually overlaps with tab content instead of appearing after the last tab.

## Root Cause

In `src/components/TabBar.ts` (line 36), the `tabsContainer` has inline style `min-width: 0`:

```js
this.tabsContainer.style.minWidth = '0';
```

In CSS flex layout, `min-width: 0` overrides the default `min-width: auto`, which normally prevents a flex item from shrinking below its content's minimum width. With `min-width: 0`:

1. The `tabsContainer` (flex: 1) shrinks to fit the viewport minus the add button (35px)
2. Individual tabs (each with CSS `min-width: 120px`) overflow the shrunken container
3. The add button is positioned after the shrunken container, not after the overflowing tabs
4. Result: add button appears in the middle of overflowing tab content

## Reproduction

```bash
npx vitest run src/components/TabBar.overflow.test.ts
```

3 of 4 tests fail, confirming the overlap at various viewport widths.

## Fix Direction

Remove `min-width: 0` from the tabsContainer inline style. With the default `min-width: auto`, the container respects its children's minimum widths, and the `.tab-bar` parent scrolls via `overflow-x: auto`.

## Regression Risk

Low — the `min-width: 0` was likely added for text truncation, but individual tabs already handle truncation via `.tab-title { overflow: hidden; text-overflow: ellipsis }`.
