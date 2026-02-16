# Tab Drag Reorder Shows Block Cursor

## Status: Investigating

## Symptom
When trying to drag tabs to reorder them, a block/forbidden (🚫) cursor icon appears immediately, preventing the drag-and-drop reorder from working.

## Root Cause Analysis

Two contributing factors identified:

### 1. Missing `dragover` handler on tab-bar container elements
The `tabsContainer` div (flex wrapper for tabs) and the outer `.tab-bar` container have no `dragover` event handler. In HTML5 DnD, any element that doesn't call `e.preventDefault()` in its `dragover` handler is considered a non-drop-target, causing the browser to show the block cursor.

When the mouse moves to any gap/empty area in the tab bar (not directly over a `.tab` element), no `preventDefault()` is called, and the block cursor appears.

**Affected code**: `src/components/TabBar.ts` — constructor (container setup) and `createTab()` (event handlers only on individual tabs).

### 2. Potential Tauri `dragDropEnabled: true` interference (Windows/WebView2)
With `dragDropEnabled: true` in `tauri.conf.json`, Tauri registers a native `IDropTarget` COM interface on the WebView2 host window. This native handler may intercept intra-webview HTML5 drag operations on Windows, returning `DROPEFFECT_NONE` for drags it doesn't recognize, which overrides the HTML5 `dropEffect` setting and shows the block cursor.

**Affected config**: `src-tauri/tauri.conf.json` line 24.

## Regression Risk
- Any change to the Tauri window configuration or WebView2 version could re-trigger this.
- Adding new elements to the tab bar without dragover handlers would also cause this.

## Reproduction
See test suite: `src/components/TabBar.drag.test.ts`
