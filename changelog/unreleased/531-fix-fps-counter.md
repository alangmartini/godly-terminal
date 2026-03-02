### Fixed
- **FPS counter always 0 in performance overlay** — PerfOverlay now runs its own requestAnimationFrame loop to count actual display frames instead of relying on data-driven render events (#531)
