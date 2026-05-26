# Tombstone: First Two Subsystems Surface Plan

Status: superseded.

This file used to drive the early proposal/candidate/run-graph and
`RunContext` implementation before the crate split settled.

Current owners:

- `docs/specs/initial_library.md` owns the product horizon.
- `docs/specs/first_two_subsystems.md` records this slice as historical
  context only.
- root `Cargo.toml` and `crates/leaven/tests/topology_contract.rs` own live
  topology.
- `crates/leaven-engine` owns `RunGraph`, graph views, stage traits, and
  `RunContext` mutation.
- `crates/leaven-core` owns only cold optimizer algebra.

Do not route new graph/runtime work from this old plan.
