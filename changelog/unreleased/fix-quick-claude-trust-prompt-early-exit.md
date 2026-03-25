### Fixed
- **Quick Claude stuck at trust prompt** — Fixed false positive early exit in `handle_trust_prompt_if_needed` that occurred when "Claude Code" appeared in trust prompt text during incremental rendering, preventing the prompt from being dismissed. Also increased startup timeout from 8s to 20s to handle slower Windows environments.
