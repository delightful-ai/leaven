# Typed Run Output Plan Tombstone

Date: 2026-05-17
Status: completed plan, landed as hard cutover on 2026-05-20.

This path used to contain the design plan for preserving typed runner outputs
through `leaven-run` scoring. The landed shape is slimmer than the draft:

- `RunOutput<Out>` preserves typed runner output through scoring.
- `Score::with_output(...)` / `Score::with_text_output(...)` are the explicit
  reportable-output boundary.
- No separate output-renderer API, renderer fingerprint axis, String default
  shim, or `TypeId` auto-render path landed.

Current truth lives in:

- `docs/specs/case_visibility_and_target_isolation.md`
- `crates/leaven-run`
- `crates/leaven-run/tests/optimize_builder.rs`
- `crates/leaven-run/tests/scoring_evaluator.rs`
- `crates/leaven-run/AGENTS.md`

Use this file as provenance only.

