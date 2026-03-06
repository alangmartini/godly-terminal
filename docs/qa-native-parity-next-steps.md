# Native QA Next Steps: E6, L4-L7, L14, L24-L26

## Scope
- E6: Visual MRU tab switcher popup.
- L4: Tab icon spacing and alignment.
- L5: `+` button hover styling.
- L6: Separator lines between inactive tabs.
- L7: Smooth tab width transitions on open/close.
- L14: Smooth sidebar collapse/expand animation on `Ctrl+B`.
- L24: Focused pane visual treatment.
- L25: Empty-state placeholder with create-terminal hint/CTA.
- L26: Terminal inset and pointer behavior around pane edges/divider.

## Preconditions
- Build under test includes E6, L4-L7, L14, and L24-L26; record commit SHA in the log below.
- Launch the native app (`godly-native`) from a clean start.
- Use one workspace with at least 5 visible tabs for the tab-bar scenarios.
- Use a second workspace, or be able to return the same workspace to 0 terminals, for the empty-state scenario.
- Be able to create at least 2 terminals in the same workspace and split them into 2 visible panes.
- Run visual checks at `1280x800` or larger unless a scenario says otherwise.
- No modal/dialog should remain open before starting keyboard or pointer checks.

## Smoke Checklist

### S1 - Open MRU popup and commit on `Ctrl` release
1. Click tab A, then tab B, then tab C so tab C is active.
2. Press and hold `Ctrl`.
3. While holding `Ctrl`, press `Tab` once.
4. Keep `Ctrl` held and observe the popup.
5. Release `Ctrl`.
Expected:
- The MRU popup opens on step 3.
- While `Ctrl` is still held, no tab switch is committed.
- Exactly 1 entry is highlighted in the popup.
- Releasing `Ctrl` closes the popup and activates the highlighted tab.

### S2 - Cycle forward and backward inside the MRU popup
1. Click tab A, then B, then C, then D so tab D is active.
2. Press and hold `Ctrl`.
3. Press `Tab` once and confirm the popup selects tab C.
4. Press `Tab` again and confirm selection moves to tab B.
5. Press `Shift+Tab` once while still holding `Ctrl` and confirm selection moves back to tab C.
6. Release `Ctrl`.
Expected:
- Selection advances one row per forward keypress and reverses one row per backward keypress.
- Only the highlighted popup row changes while the popup is open; the active tab does not change yet.
- Releasing `Ctrl` commits the currently highlighted tab and closes the popup.

### S3 - Cancel the MRU popup with `Escape`
1. Click tab A, then tab B, then tab C so tab C is active.
2. Press and hold `Ctrl`.
3. Press `Tab` once to open the popup.
4. Press `Escape` while the popup is visible.
5. Release `Ctrl`.
Expected:
- The popup closes immediately on `Escape`.
- No tab switch is committed after cancel.
- Tab C remains active.

### S4 - Tab icon alignment and `+` hover polish
1. Resize the window to `1280x800`.
2. Ensure at least 4 tabs are visible at once, including tabs with and without process icons.
3. Hover each visible tab close button, then move the pointer away.
4. Hover the `+` button, then move the pointer away.
Expected:
- Process icon, tab title, and close button stay vertically centered in one row.
- No overlap or clipping appears between icon, title, and close button.
- The `+` button shows a clear hover state and returns cleanly on mouse-out.
- Hovering any of these controls does not shift surrounding tab layout.

### S5 - Inactive-tab separators render correctly
1. Ensure at least 5 tabs are visible.
2. Make a middle tab active and scan the separators on both sides.
3. Make the first visible tab active and scan the separators again.
4. Make the last visible tab active and scan the separators again.
Expected:
- Thin separators appear only between inactive tabs.
- Separators are not drawn through the active tab.
- Active-tab changes do not produce doubled or thick separator artifacts.

### S6 - Tab width transitions on open and close
1. Start with at least 4 visible tabs.
2. Click the `+` button once to open a new tab.
3. Watch the width change across the full tab row while the new tab appears.
4. Close a middle tab.
5. Watch the width change across the remaining tabs while the row settles.
Expected:
- Opening a tab causes neighboring tabs to resize smoothly instead of jumping to their final widths in one frame.
- Closing a tab causes the remaining tabs to expand smoothly.
- No tab text, icon, or close button overlaps during the transition.
- No flicker, separator glitches, or one-frame layout collapse appears during open or close.

### S7 - Sidebar collapse and expand animation on `Ctrl+B`
1. Start with the sidebar expanded and at least 2 workspace rows visible.
2. Press `Ctrl+B` once.
3. Watch the sidebar until it finishes collapsing.
4. Press `Ctrl+B` again.
5. Watch the sidebar until it finishes expanding.
Expected:
- A visible collapse animation runs on step 2 and a visible expand animation runs on step 4.
- The sidebar width changes smoothly; it does not snap open or closed in a single frame.
- Workspace content, tab bar, and terminal area reflow without flicker or overlap.
- The final collapsed and expanded states remain fully usable.

### S8 - Focused pane treatment follows the active split
1. Open 2 terminals in the same workspace.
2. Split them into 2 visible panes.
3. Click inside the left pane.
4. Click inside the right pane.
5. Click back inside the left pane.
Expected:
- Exactly 1 pane shows the focused border/glow treatment at any time.
- The focused treatment moves immediately to the clicked pane.
- The previously focused pane loses the treatment when focus changes.
- No stale highlight remains on the previously focused pane.

### S9 - Empty state shows `No terminals open` and create hint
1. Open a workspace with 0 terminals, or close/remove the last terminal until the workspace is empty.
2. Observe the terminal area before clicking anything.
3. Activate the empty-state create CTA or hint once.
Expected:
- The empty state is visible immediately when the workspace reaches 0 terminals.
- The headline reads `No terminals open`.
- A visible create-terminal hint or CTA is present.
- Activating it creates exactly 1 terminal and removes the empty state.

### S10 - Terminal inset is visible and pointer-safe
1. Ensure exactly 1 terminal is visible in the workspace.
2. Resize the window to `1280x800`.
3. Visually inspect the terminal area on all 4 sides.
4. Move the pointer along the top inset, then the right inset, then into the rendered terminal content.
5. Click once near the top inset, once near the right inset, and once inside the terminal content.
Expected:
- A small, consistent inset is visible between pane chrome and terminal content on all 4 sides.
- Terminal text and cursor are not flush against the pane edge and are not clipped.
- The inset does not create dead pointer zones; all clicks still target the terminal correctly.
- Pointer movement across the inset does not reveal stray overlays, misaligned hitboxes, or hover artifacts.

### S11 - Divider hitbox remains correct with inset enabled
1. Return to a 2-pane split.
2. Move the pointer onto the divider between the panes.
3. Verify the divider hover/resize affordance appears.
4. Drag the divider slightly, then release.
5. Click near each pane's divider-side inset.
Expected:
- The divider remains easy to hit even with terminal inset enabled.
- The terminal surface does not cover the divider hitbox or block divider drag.
- Dragging the divider resizes the panes without blocked pointer events or stray hover artifacts.
- Clicking near the divider-side inset still focuses the intended pane.

## Failure Logging Template
Use one entry per failing scenario.

```text
Title: [E6/L4-L7/L14/L24-L26][Sx] Short summary
Date/Time:
Tester:
Build/Commit SHA:
OS:
Scenario ID: (S1-S11)
Preconditions met: Yes/No
Window size:
Workspace state: (tab count, active tab position, terminal count, split orientation, sidebar expanded/collapsed)
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
