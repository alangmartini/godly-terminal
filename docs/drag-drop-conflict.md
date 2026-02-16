# Drag-and-Drop Conflict: Internal DnD vs External File Drops

## Problem

`dragDropEnabled: true` in `src-tauri/tauri.conf.json` registers Tauri's native `IDropTarget`
on the WebView2 window. This intercepts the OLE drag pipeline, which WebView2 uses even for
intra-webview HTML5 drags. Result: external file drops (ShareX screenshots -> paste file path)
work, but internal HTML5 DnD (tab reorder, workspace reorder, split zones) is completely broken.

## Root Cause

Tauri's `IDropTarget` and WebView2's HTML5 DnD both use the Windows OLE drag-and-drop pipeline.
They are mutually exclusive by design.

## Solution

Keep `dragDropEnabled: true` for file drops. Replace all internal HTML5 DnD with pointer-event-based
drag (`pointerdown`/`pointermove`/`pointerup`), which operates outside the OLE pipeline.

## Affected Components

- `TabBar.ts` - tab reordering
- `WorkspaceSidebar.ts` - workspace reordering + cross-workspace tab moves
- `App.ts` - split drop zones

## Regression Risk

High. Any reintroduction of HTML5 `draggable`/`ondragstart`/`ondrop` on internal elements will
break again while `dragDropEnabled: true`.
