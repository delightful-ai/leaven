# Tombstone: Result Facade And GEPA Ergonomics Decisions

Status: implemented and superseded.

This file used to record the decision to cut over from `OptimizeResult` to the
ordinary `Optimized<A>` result facade and to keep GEPA ergonomics out of the
ordinary prelude.

Current owners:

- `docs/specs/gepa_reference_behavior.md`
- `docs/specs/gepa_public_private_surface.md`
- `crates/leaven-run`
- `crates/leaven`
- `crates/leaven-gepa`
- `crates/leaven/tests/public_surface_contract.rs`

Use the live public surface tests and crate docs before changing result or GEPA
routes.
