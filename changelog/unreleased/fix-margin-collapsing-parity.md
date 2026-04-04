### Fixed

- **Transcript block spacing parity** — Reference pane now simulates CSS margin collapsing (gap = max(prev_bottom, curr_top)) instead of additive flex margins, eliminating ~82px of cumulative vertical drift across 15 inter-block gaps in the transcript layout.
