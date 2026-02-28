### Fixed

- **Stale terminal IDs in persisted layout** — Layout trees, tab orders, split views, and zoomed panes could hold terminal IDs that become invalid after app crash or dirty exit, causing phantom panes and `self_split` failures. Added centralized pruning that runs after terminal restoration to remove dead references. (#447)
