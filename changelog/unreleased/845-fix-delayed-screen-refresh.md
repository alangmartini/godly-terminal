### Fixed

- **Delayed screen refresh / micro-blinking** — Fixed two issues causing visual instability: (1) render throttle now applies universally to all pixel renders (heartbeat-driven fetches every 16ms were completely unthrottled, causing 60fps texture churn); (2) heartbeat recovery polls now skip terminals fetched within the last 200ms to avoid adding extra renders during continuous output. Pixel render rate capped at ~20fps during active output. (#845)
