# Native QA Smoke: L14 + L24-L26 Sidebar and Terminal Polish

## Scope
- L14: Sidebar collapse/expand animation on `Ctrl+B`.
- L24: Focused split pane visual treatment.
- L25: Empty-state placeholder and create-terminal CTA.
- L26: Terminal inset and pointer behavior around pane edges/dividers.

## Preconditions
- Build under test includes the L14 and L24-L26 changes; record commit SHA in the log below.
- Launch the native app (`godly-native`) from a clean start.
- Use one workspace that can create at least 2 terminals and supports split panes.
- Be able to reach an empty workspace state with 0 terminals open.
- Prefer a dedicated empty workspace for scenarios S2-S5 so the state can build forward.
- Run the checks at `1280x800` or larger unless a scenario says otherwise.
- No modal/dialog should remain open before starting shortcut or pointer checks.

## Smoke Checklist

### S1 - Sidebar animation on `Ctrl+B`
1. Start with the sidebar expanded, at least 2 workspace rows visible, and keyboard focus inside the main window.
2. Press `Ctrl+B` once.
3. Watch the sidebar until it finishes collapsing.
4. Press `Ctrl+B` again.
5. Watch the sidebar until it finishes expanding.
Expected:
- A visible collapse animation runs on step 2 and a visible expand animation runs on step 4.
- The sidebar width changes smoothly; it does not jump open/closed in a single frame.
- Workspace content, tab bar, and terminal area reflow without flicker or overlap during both transitions.
- The final collapsed and expanded states are stable and fully usable.

### S2 - Empty state shows headline and create CTA
1. Open a workspace with 0 terminals, or close/remove the last terminal until the workspace is empty.
2. Observe the terminal area before clicking anything.
3. Activate the empty-state create CTA once.
Expected:
- The empty state is visible immediately after the workspace reaches 0 terminals.
- The headline reads `No terminals open`.
- A visible create-terminal CTA or hint is present in the empty state.
- Activating the CTA creates exactly 1 terminal, removes the empty state, and returns the workspace to a usable terminal view.

### S3 - Terminal inset is visible and does not create dead pointer zones
1. Ensure exactly 1 terminal is visible in the workspace.
2. Resize the window to `1280x800`.
3. Visually inspect the terminal area on all 4 sides.
4. Move the pointer along the top inset, then the right inset, then into the rendered terminal content.
5. Click once near the top inset, once near the right inset, and once inside the terminal content.
Expected:
- A small, consistent inset is visible between the pane chrome and the rendered terminal content on all 4 sides.
- Text/cursor content is not flush against the pane edge and is not clipped.
- The inset does not introduce dead pointer zones; all clicks still target the terminal correctly.
- Pointer movement across the inset does not reveal stray overlays, misaligned hitboxes, or hover artifacts.

### S4 - Focused pane visual follows the active split
1. Split the current workspace into 2 visible panes.
2. Click inside the left pane.
3. Click inside the right pane.
4. Click back inside the left pane.
Expected:
- Exactly 1 pane shows the focused treatment at any time.
- The focused border/glow moves immediately to the pane clicked in steps 2-4.
- The previously focused pane loses the treatment when focus changes.
- No pane keeps a stale highlight after focus moves.

### S5 - Divider hitbox stays correct with inset enabled
1. Return to a 2-pane split.
2. Move the pointer onto the divider between the panes.
3. Verify the divider hover/resize affordance appears.
4. Drag the divider slightly, then release.
5. Click back into each pane near the divider-side inset.
Expected:
- The divider remains easy to hit even though each pane has terminal inset.
- The terminal surface does not cover the divider hitbox or block divider drag.
- Dragging the divider resizes the panes without blocked pointer events or stray hover artifacts.
- Clicking near the divider-side inset still focuses the intended pane.

## Failure Logging Template
Use one entry per failing scenario.

```text
Title: [L14/L24/L25/L26][Sx] Short summary
Date/Time:
Tester:
Build/Commit SHA:
OS:
Scenario ID: (S1-S5)
Preconditions met: Yes/No
Workspace state: (sidebar expanded/collapsed, terminal count, split orientation)
Window size:
Exact interaction sequence:
Expected result:
Actual result:
Repro rate: (1/1, 3/5, etc.)
Artifacts:
- Screenshot path:
- Video path:
- Native app stdout/stderr capture path:
Notes:
```
