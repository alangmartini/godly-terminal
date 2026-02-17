# Split view destroyed when switching tabs

## Status: RESOLVED

## Symptoms
When two terminals are in a split view and the user switches to a third terminal tab, then switches back to either split terminal, the split view is gone.

## Root Cause
`store.setActiveTerminal()` permanently destroyed the split when navigating to a terminal outside of it. The split was removed from `state.splitViews` and `clear_split_view` was called on the backend, with no way to recover the split.

## Fix
Added a `suspendedSplitViews` map to the Store class. When navigating away from a split, the split is saved to `suspendedSplitViews` instead of being permanently destroyed. When navigating back to a terminal that was part of a suspended split, the split is restored (both frontend state and backend via `set_split_view`).

Suspended splits are properly cleaned up when:
- A terminal in the suspended split is removed
- A terminal in the suspended split is moved to another workspace
- The workspace is removed
- `clearSplitView()` is called explicitly
- `reset()` is called

## Files Changed
- `src/state/store.ts` - Added `suspendedSplitViews` map, modified `setActiveTerminal()` to suspend/restore

## Test Coverage
- `src/state/store.split-navigation.test.ts` - 7 tests covering restore on left/right terminal, direction/ratio preservation, multi-tab navigation, and edge cases (terminal closed, both closed, new split after restore)

## Regression Risk
Medium - the `setActiveTerminal` method is called on every tab switch. The suspended split logic adds a map lookup on each call.
