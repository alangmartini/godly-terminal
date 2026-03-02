### Changed
- **FairMutex for VT parser** — Replaced standard Mutex with parking_lot::FairMutex for the terminal parser, preventing snapshot request starvation under heavy output (refs #511)
