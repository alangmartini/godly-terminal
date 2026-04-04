# Test Spec — godly-shell visual quality uplift

## Scope under test
- Deterministic reference-mode rendering quality for `godly-shell`
- Touchpoints likely in transcript typography/compositing and supporting chrome spacing

## Verification evidence
1. Baseline and post-change parity run:
   - `powershell -ExecutionPolicy Bypass -File scripts/measure-godly-shell-parity.ps1`
2. Build/type validation:
   - `cd src-tauri && cargo check -p godly-shell`
3. Targeted Rust tests for touched UI/layout modules:
   - Example: `cd src-tauri && cargo test -p godly-shell ui::reference_layout -- --nocapture`
   - Add module-specific tests if new logic is introduced.
4. Diagnostics/readout:
   - Review command output directly; no completion claim without fresh passing evidence.

## Pass criteria
- No new `cargo check -p godly-shell` failures.
- Touched-module tests pass.
- Parity harness completes successfully and does not show a worse regression versus baseline; improvement is preferred and should be reported numerically.

## Risk areas
- Font rasterization/compositing changes can improve one region while hurting another.
- Screenshot parity may vary if layout or capture assumptions drift.
- Transcript and chrome are tightly coupled through shared font helpers.
