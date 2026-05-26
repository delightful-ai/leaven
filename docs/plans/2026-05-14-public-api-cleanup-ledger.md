# Tombstone: Public API Cleanup Orchestration Ledger

Status: completed and superseded.

This file used to track the 2026-05-14 public API cleanup workstream. Its
follow-up result-facade and GEPA ergonomics slices have landed, so the old
ledger no longer carries current next actions.

Current owners:

- `crates/leaven/tests/public_surface_contract.rs` owns umbrella route
  classification and drift checks.
- `crates/leaven/AGENTS.md` owns route policy for `prelude`, `extend`, and
  `plumbing`.
- `crates/leaven-run` owns the `Optimized` result facade and run summaries.
- `crates/leaven-gepa` owns GEPA reflection and ergonomic constructors.
- `docs/specs/gepa_reference_behavior.md` owns current GEPA truth.

Use live tests and specs above before changing public routes.
