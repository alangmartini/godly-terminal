# Native QA Smoke: E6 Popup + L4-L6 Tab Bar Polish

## Scope
- E6: MRU tab switcher popup behavior.
- L4: Tab icon spacing/alignment.
- L5: "+" button hover styling.
- L6: Separator lines between inactive tabs.

## Preconditions
- Build under test includes the E6 and L4-L6 changes (record commit SHA in the log below).
- Run the native app (`godly-native`) from a clean start.
- Use one workspace with at least 4 visible tabs.
- Tabs must include mixed labels (short + long), and at least one tab with a process icon.
- At least 3 tabs must be inactive at the same time (for separator checks).
- No modal/dialog is open before starting key-sequence tests.

## Smoke Checklist

### S1 - Open popup and commit on Ctrl release
1. Click tab A, then tab B, then tab C (C is active).
2. Press and hold `Ctrl`.
3. While holding `Ctrl`, press `Tab` once (`Ctrl+Tab`).
4. Keep `Ctrl` held: verify popup is visible and selection moved to the next MRU target.
5. Release `Ctrl`.
Expected:
- Popup opens on step 3.
- No commit while `Ctrl` is still held.
- On `Ctrl` release, popup closes and selected tab becomes active.

### S2 - Cycle forward with repeated Ctrl+Tab
1. Click tab A, then tab B, then tab C, then tab D (D is active, MRU = D, C, B, A).
2. Press and hold `Ctrl`.
3. Press `Tab` once (`Ctrl+Tab`): verify C is selected in the popup.
4. Press `Tab` two more times (still holding `Ctrl`): verify selection moves to B, then A.
5. Release `Ctrl`.
Expected:
- Selection advances one row per `Tab` press in this exact order: C -> B -> A.
- Exactly one row is highlighted at any time.
- Active tab changes only after `Ctrl` release, and tab A becomes active.

### S3 - Cycle backward with Ctrl+Shift+Tab
1. Click tab A, then tab B, then tab C, then tab D (D is active, MRU = D, C, B, A).
2. Press and hold `Ctrl`.
3. Press and hold `Shift`.
4. Press `Tab` once (`Ctrl+Shift+Tab`): verify A is selected in the popup.
5. Press `Tab` once more (still holding `Ctrl+Shift`): verify selection moves to B.
6. Release `Ctrl`, then release `Shift`.
Expected:
- Selection moves backward in MRU order.
- Commit happens on `Ctrl` release.
- Popup closes after commit, and tab B becomes active.

### S4 - Cancel popup with Escape
1. Click tab A, then tab B, then tab C (C is active).
2. Press and hold `Ctrl`.
3. Press `Tab` once to open popup (`Ctrl+Tab`).
4. Press `Escape` while popup is visible (keep `Ctrl` held).
5. Release `Ctrl`.
Expected:
- Popup closes immediately on `Escape`.
- No tab switch is committed after cancel.
- Previously active tab (C) remains active.

### S5 - L4/L5 visual polish
1. Resize window to `1280x800`.
2. Ensure at least 4 tabs are visible at once, including tabs with and without process icons.
3. Hover each visible tab close button, then move pointer away.
4. Hover the `+` button, then move pointer away.
Expected:
- Icon, title text, and close button are vertically centered in the same row.
- No overlap/clipping between icon, title, and close button at `1280x800`.
- `+` button has a visible hover state and returns cleanly on mouse-out.
- Hover does not cause tab bar layout shift.

### S6 - L6 inactive-tab separators
1. Ensure at least 5 tabs are visible.
2. Make a middle tab active and scan separators.
3. Make the first visible tab active and scan separators.
4. Make the last visible tab active and scan separators.
Expected:
- Thin separators are visible between inactive tabs.
- Separators are not drawn through the active tab.
- No doubled/thick separator artifacts during active-tab changes (middle -> first -> last).

## Failure Logging Template
Use one entry per failing scenario.

```text
Title: [E6/L4/L5/L6][Sx] Short summary
Date/Time:
Tester:
Build/Commit SHA:
OS:
Scenario ID: (S1-S6)
Preconditions met: Yes/No
Exact interaction sequence: (example: Click A -> Click B -> Hold Ctrl -> Tab -> Release Ctrl)
Window size:
Visible tab count:
Active tab position: (first/middle/last)
Expected result:
Actual result:
Repro rate: (1/1, 3/5, etc.)
Artifacts:
- Screenshot path:
- Video path:
- Native app stdout/stderr capture path:
Notes:
```
