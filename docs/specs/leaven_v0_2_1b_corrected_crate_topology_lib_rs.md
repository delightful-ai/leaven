# Leaven v0.2.3 Topology Tombstone

This file used to contain the v0.2.1b/v0.2.3 pre-implementation crate
topology draft. It listed placeholder crates and `lib.rs` maps that no longer
match the implemented workspace.

The current topology truth is:

- root `Cargo.toml` for workspace membership and dependency declarations
- `crates/leaven/tests/topology_contract.rs` for executable ownership,
  dependency, public-route, and deleted-placeholder checks
- root `AGENTS.md` plus the nearest crate `AGENTS.md` for routing rules

Do not use the old draft as crate inventory, public maturity evidence, or a
reason to reintroduce placeholder crates. Reintroduce deleted crate names only
with behavior-bearing code, tests, topology rows, and local ownership docs.

