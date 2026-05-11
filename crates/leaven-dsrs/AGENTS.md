## Boundary
This directory is quarantined bait. It is not a current Cargo workspace crate:
there is no `Cargo.toml`, no `src/lib.rs`, and no topology-contract coverage.

The files here are placeholders only. Do not route new DSRS work here unless
the crate is deliberately reintroduced.

## What Exists Here
- `src/artifact.rs`: `DsrsProgramArtifact` and `DsrsProgramChange` placeholders.
- `src/surface.rs`: `DsrsProgramSurface` placeholder.
- `src/evaluator.rs`: `DsrsEvaluator` placeholder.
- `src/bridge.rs`: `DsrsSignatureBridge` placeholder.

These names are not public API because there is no crate root. They are still
dangerous because specs and directory presence can make them look like an
ownership target.

## Route Away
- Generic artifact and proposal vocabulary belongs in `crates/leaven-core`.
- Concrete artifact families belong in `crates/leaven-artifacts` or a real
  `crates/leaven-artifact-*` workspace crate with tests.
- Surfaces belong in `crates/leaven-surface` or a concrete artifact crate.
- Evaluator execution belongs in `crates/leaven-engine`, `crates/leaven-eval`,
  or a real adapter crate named by the governing spec.

## Reintroduction Gate
If DSRS becomes a real Leaven adapter again, add the manifest, `src/lib.rs`,
workspace membership, topology-contract assertions, governing spec update,
tests, and this file's new local boundary in the same coherent change.

Until then, the correct action is to route to the current owning crate or write
a dated plan/spec for reintroduction, not to expand these placeholder files.

## Proof Anchor
`cargo test -p leaven --test topology_contract` currently proves workspace
membership and dependency topology, but audit docs note it should also reject
unregistered `crates/*` directories. Until that gate exists, this `AGENTS.md`
is the local tripwire.
