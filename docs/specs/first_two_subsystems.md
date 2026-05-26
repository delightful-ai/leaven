# First Two Subsystems Tombstone

Status: superseded historical sketch.

This path used to contain a v0.2.1-oriented implementation sketch for proposal,
candidate, `RunGraph`, and `RunContext` surfaces. The implemented workspace has
since moved through later topology, engine, stage, run, and public-seam work.

Current truth lives in:

- `docs/specs/initial_library.md` for product horizon
- root `Cargo.toml` and `crates/leaven/tests/topology_contract.rs` for live
  crate topology
- `crates/leaven-core`, `crates/leaven-engine`, and `crates/leaven-run` for
  the owning Rust surfaces and contract tests
- `docs/specs/public-seam-v1` for external-language worker seam semantics

Do not implement from the old sketch. If a historical review cites this path,
use it as provenance only, then verify the current code, specs, and tests.

