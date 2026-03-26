### Fixed
- **Quick Claude trust prompt stuck** — trust prompt auto-accept now retries with a stabilization delay, fixing race condition where the Enter keypress arrived before Claude Code's input handler was ready (#825)
