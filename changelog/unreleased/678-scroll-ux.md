### Fixed
- **Mouse wheel jitter from fractional delta truncation** — small trackpad deltas (0.1–0.3) are now accumulated across events instead of truncated to zero, eliminating rollback jitter (#678)
- **Missing scrollbar in terminal pane** — integrated `scrollbar::view_scrollbar()` into `render_terminal_pane()` to render the scrollbar next to the terminal canvas (#678)

### Changed
- **Shift+Home/End keybindings** — added Shift+Home for ScrollToTop and Shift+End for ScrollToBottom, providing standard terminal scroll keybindings (#678)
